"""Export the MPFB character (character_mpfb.blend) to assets/player.bin for the Island game.

Run:  blender --background --python tools/export_player.py -- <character.blend> <out player.bin>

Output (little endian):
  "ISPL", u32 nverts, u32 ntris, u32 njoints
  njoints * 3 f32   rest-pose joints, game space (x forward, y up, z right)
  nverts * 3 f32    positions
  nverts * 3 f32    normals
  nverts * 3 u8     colours (sRGB, baked from the textures)
  nverts * 4 u8     bone indices
  nverts * 4 u8     bone weights (0..255, sum 255)
  ntris * 3 u16     indices
Game bones: 0 pelvis 1 chest 2 head, then pairs (right, left): 3/4 upper arm, 5/6 forearm, 7/8 hand,
            9/10 thigh, 11/12 calf, 13/14 foot.
"""
import bpy, sys, struct, math
import numpy as np
from mathutils import Vector, kdtree

blend, out_path = sys.argv[sys.argv.index("--") + 1:][:2]
bpy.ops.wm.open_mainfile(filepath=blend)

rig = bpy.data.objects["Body.rig"]
rig.data.pose_position = "REST"
body_obj = bpy.data.objects["Body"]

SKIP_MATS = {"Body.nipple", "Body.genitals", "Body.toenails"}
SKIP_OBJS = {"Body.teeth_base"}


def g(v):  # blender (x right-of-model-left, -y forward, z up) -> game (x forward, y up, z right)
    return (-v[1], v[2], -v[0])


# ---------------------------------------------------------------- rest joints
J = [("thigh_r", "head"), ("thigh_l", "head"), ("calf_r", "head"), ("calf_l", "head"),
     ("foot_r", "head"), ("foot_l", "head"), ("ball_r", "head"), ("ball_l", "head"),
     ("upperarm_r", "head"), ("upperarm_l", "head"), ("lowerarm_r", "head"), ("lowerarm_l", "head"),
     ("hand_r", "head"), ("hand_l", "head"), ("middle_03_r", "tail"), ("middle_03_l", "tail"),
     ("pelvis", "head"), ("spine_03", "head"), ("head", "head"), ("head", "tail")]
rest_joints = []
for name, end in J:
    b = rig.data.bones[name]
    p = rig.matrix_world @ (b.head_local if end == "head" else b.tail_local)
    rest_joints.append(g(p))


# ---------------------------------------------------------------- bone mapping
def game_bone(name):
    n = name.lower()
    side = 0 if (n.endswith("_r") or ".r" in n) else 1 if (n.endswith("_l") or ".l" in n) else None
    if n in ("root", "pelvis", "spine_01"):
        return 0
    if n in ("spine_02", "spine_03") or n.startswith("clavicle"):
        return 1
    if n in ("neck_01", "head"):
        return 2
    if side is None:
        return None
    if n.startswith("upperarm"):
        return 3 + side
    if n.startswith("lowerarm"):
        return 5 + side
    if n.startswith(("hand", "index", "middle", "ring", "pinky", "thumb")):
        return 7 + side
    if n.startswith("thigh"):
        return 9 + side
    if n.startswith("calf"):
        return 11 + side
    if n.startswith(("foot", "ball")):
        return 13 + side
    return None


# ---------------------------------------------------------------- weights source (unmasked body at rest)
src = body_obj.data
kd = kdtree.KDTree(len(src.vertices))
for i, v in enumerate(src.vertices):
    kd.insert(body_obj.matrix_world @ v.co, i)
kd.balance()
vg_names = [vg.name for vg in body_obj.vertex_groups]
w_cache = {}


def weights_for(co_world):
    _, idx, _ = kd.find(co_world)
    if idx in w_cache:
        return w_cache[idx]
    acc = {}
    for ge in src.vertices[idx].groups:
        b = game_bone(vg_names[ge.group])
        if b is not None and ge.weight > 0:
            acc[b] = acc.get(b, 0.0) + ge.weight
    if not acc:
        acc = {0: 1.0}
    top = sorted(acc.items(), key=lambda kv: -kv[1])[:4]
    tot = sum(w for _, w in top)
    ws = [(b, w / tot) for b, w in top]
    q = [int(round(w * 255)) for _, w in ws]
    q[0] += 255 - sum(q)
    bones = [b for b, _ in ws] + [0] * (4 - len(ws))
    q = q + [0] * (4 - len(q))
    w_cache[idx] = (bones, q)
    return w_cache[idx]


# ---------------------------------------------------------------- colours
img_cache = {}


def image_array(img):
    if img.name in img_cache:
        return img_cache[img.name]
    im = img.copy()
    # read the raw 8-bit values (no colour management), then linearise them ourselves
    im.colorspace_settings.name = "Non-Color"
    if max(im.size) > 1024:
        im.scale(1024, 1024)
    w, h = im.size
    arr = np.empty(w * h * im.channels, dtype=np.float32)
    im.pixels.foreach_get(arr)
    arr = arr.reshape(h, w, im.channels)
    rgb = arr[..., :3]
    arr[..., :3] = np.where(rgb <= 0.04045, rgb / 12.92, np.power((rgb + 0.055) / 1.055, 2.4))
    img_cache[img.name] = arr
    return arr


def iter_nodes(tree, depth=0):
    for n in tree.nodes:
        yield n
        if n.type == "GROUP" and n.node_tree and depth < 4:
            yield from iter_nodes(n.node_tree, depth + 1)


picked_log = {}


def pick_image(mat):
    if not mat or not mat.use_nodes:
        return None
    img = _pick_image(mat)
    if mat.name not in picked_log:
        picked_log[mat.name] = img.name if img else None
        print("PICK", mat.name, "->", img.name if img else None)
    return img


def _pick_image(mat):
    best = None
    for n in iter_nodes(mat.node_tree):
        if n.type == "TEX_IMAGE" and n.image:
            nm = n.image.name.lower()
            if any(k in nm for k in ("normal", "bump", "spec", "rough", "ao", "sss", "mask")):
                continue
            if "diffuse" in nm or "color" in nm or "albedo" in nm:
                return n.image
            best = best or n.image
    return best


def base_color(mat):
    if mat and mat.use_nodes:
        for n in mat.node_tree.nodes:
            if n.type == "BSDF_PRINCIPLED":
                c = n.inputs["Base Color"].default_value
                return np.array(c[:3], dtype=np.float32)
    return np.array([0.6, 0.6, 0.6], dtype=np.float32)


def lum(c):
    return float(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2])


def lerp(a, b, t):
    return np.array(a, dtype=np.float32) * (1 - t) + np.array(b, dtype=np.float32) * t


def color_at(mat, uv):
    img = pick_image(mat)
    if img is not None and uv is not None:
        a = image_array(img)
        h, w = a.shape[:2]
        c = a[int((uv[1] % 1.0) * h) % h, int((uv[0] % 1.0) * w) % w, :3]
    else:
        c = base_color(mat)
    name = mat.name if mat else ""
    # clothes are flat colours: the printed logo / fabric folds in the textures turn into blotches
    # when baked onto vertices
    if name == "Tee":      # light-blue tee
        c = np.array([0.55, 0.70, 0.90], dtype=np.float32)
    elif name == "Trousers":  # grey trousers
        c = np.array([0.13, 0.13, 0.14], dtype=np.float32)
    elif name == "Body.short02":  # bleached hair
        c = lerp(c, (0.95, 0.84, 0.52), 0.62)
    return np.clip(c, 0, 1)


def to_srgb8(c):
    # image.pixels / node colours are scene-linear; the game's colours are display values
    s = np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(np.maximum(c, 1e-6), 1 / 2.4) - 0.055)
    return [int(round(float(x) * 255)) for x in np.clip(s, 0, 1)]


# ---------------------------------------------------------------- gather geometry
dg = bpy.context.evaluated_depsgraph_get()
for ob in bpy.data.objects:
    if ob.type == "MESH" and ob is not body_obj:
        for m in ob.modifiers:
            if m.type == "SUBSURF":
                m.show_viewport = False
dg.update()

positions, normals, colors, bidx, bwts, tris = [], [], [], [], [], []
keymap = {}
for ob in bpy.data.objects:
    if ob.type != "MESH" or ob.name in SKIP_OBJS or ob.hide_get():
        continue
    first = len(colors)
    ev = ob.evaluated_get(dg)
    me = ev.to_mesh()
    me.calc_loop_triangles()
    uv_layer = me.uv_layers.active
    mw = ob.matrix_world
    nm = mw.to_3x3().inverted_safe().transposed()
    used = 0
    # accumulate colour per (vertex, material) from its loops
    acc = {}
    for lt in me.loop_triangles:
        mat = ob.material_slots[me.polygons[lt.polygon_index].material_index].material if ob.material_slots else None
        if mat and mat.name in SKIP_MATS:
            continue
        for li, vi in zip(lt.loops, lt.vertices):
            uv = tuple(uv_layer.data[li].uv) if uv_layer else None
            c = color_at(mat, uv)
            a = acc.setdefault((vi, mat.name if mat else ""), [np.zeros(3, np.float32), 0, mat])
            a[0] += c
            a[1] += 1
    for lt in me.loop_triangles:
        mat = ob.material_slots[me.polygons[lt.polygon_index].material_index].material if ob.material_slots else None
        if mat and mat.name in SKIP_MATS:
            continue
        ids = []
        for vi in lt.vertices:
            key = (ob.name, vi, mat.name if mat else "")
            if key not in keymap:
                co = mw @ me.vertices[vi].co
                n = (nm @ me.vertices[vi].normal).normalized()
                a = acc[(vi, mat.name if mat else "")]
                col = a[0] / max(a[1], 1)
                b, w = weights_for(co)
                keymap[key] = len(positions)
                positions.append(g(co))
                normals.append(g(n))
                colors.append(to_srgb8(col))
                bidx.append(b)
                bwts.append(w)
            ids.append(keymap[key])
        tris.append(ids)
        used += 1
    print("OBJ", ob.name, "tris", used, "mean rgb", np.mean(np.array(colors[first:]), axis=0).round(0) if len(colors) > first else None)
    ev.to_mesh_clear()

nv, nt = len(positions), len(tris)
print("TOTAL verts", nv, "tris", nt)
assert nv < 65536
with open(out_path, "wb") as f:
    f.write(b"ISPL")
    f.write(struct.pack("<III", nv, nt, len(rest_joints)))
    for j in rest_joints:
        f.write(struct.pack("<3f", *j))
    for p in positions:
        f.write(struct.pack("<3f", *p))
    for n in normals:
        f.write(struct.pack("<3f", *n))
    for c in colors:
        f.write(struct.pack("<3B", *c))
    for b in bidx:
        f.write(struct.pack("<4B", *b))
    for w in bwts:
        f.write(struct.pack("<4B", *w))
    for t in tris:
        f.write(struct.pack("<3H", *t))
xs = [p[0] for p in positions]; ys = [p[1] for p in positions]; zs = [p[2] for p in positions]
print("BOUNDS x", min(xs), max(xs), "y", min(ys), max(ys), "z", min(zs), max(zs))
print("REST JOINTS", [tuple(round(c, 3) for c in j) for j in rest_joints])
print("MEAN COLOUR", np.mean(np.array(colors), axis=0))
print("WROTE", out_path)
