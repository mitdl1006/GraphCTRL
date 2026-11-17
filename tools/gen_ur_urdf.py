"""Writes a UR e-Series URDF from the upstream YAML files in assets/ur/config, as in robots/<model>/.

Usage: python3 tools/gen_ur_urdf.py assets/ur <model> visual > robots/<model>/<model>.urdf, then point the
mesh paths into the package: package://<model>/meshes/{visual,collision}/<part>.
"""
import math, sys, yaml

def degrees(loader, node):
    return math.radians(float(loader.construct_scalar(node)))
yaml.SafeLoader.add_constructor('!degrees', degrees)

def clean(v):
    v = float(v)
    for k in range(-4, 5):
        if abs(v - k * math.pi / 2) < 1e-8:
            return k * math.pi / 2
    if abs(v) < 1e-9:
        return 0.0
    d = math.degrees(v)
    if abs(d) >= 1 and abs(d - round(d)) < 1e-9:
        return math.radians(round(d))
    return round(v, 10)

def num(v):
    v = clean(v)
    if v == 0:
        return "0"
    r = repr(v)
    return r[:-2] if r.endswith(".0") else r

def xyz(d): return " ".join(num(d[k]) for k in ("x", "y", "z"))
def rpy(d): return " ".join(num(d[k]) for k in ("roll", "pitch", "yaw"))

def main(root, model, visual):
    cfg = f"{root}/config/{model}"
    kin = yaml.safe_load(open(f"{cfg}/default_kinematics.yaml"))["kinematics"]
    lim = yaml.safe_load(open(f"{cfg}/joint_limits.yaml"))["joint_limits"]
    vis = yaml.safe_load(open(f"{cfg}/visual_parameters.yaml"))["mesh_files"]
    out = ['<?xml version="1.0"?>',
           f'<!-- {model.upper()[:-1]}e: joint origins, limits and collision meshes as published in Universal_Robots_ROS2_Description 4.4.1',
           '     (BSD-3-Clause, see assets/ur/upstream/LICENSE). Generated from its YAML files; '
           + ('visual meshes are drawn; the simpler collision meshes are checked. -->' if visual else
              'no visual meshes are bundled, so the collision meshes are drawn and checked. -->'),
           f'<robot name="{model}">', '  <link name="base_link"/>']
    links = [("base_link_inertia", "base"), ("shoulder_link", "shoulder"), ("upper_arm_link", "upper_arm"),
             ("forearm_link", "forearm"), ("wrist_1_link", "wrist_1"), ("wrist_2_link", "wrist_2"),
             ("wrist_3_link", "wrist_3")]
    for link, part in links:
        m = vis[part]
        off = m["mesh_offset"]
        out.append(f'  <link name="{link}">')
        kinds = (("visual", "visual"), ("collision", "collision")) if visual else (("collision", "collision"),)
        for tag, key in kinds:
            path = m[key]["mesh"]["path"]
            out += [f'    <{tag}>', f'      <origin xyz="{xyz(off)}" rpy="{rpy(off)}"/>',
                    f'      <geometry><mesh filename="package://ur_description/{path}"/></geometry>', f'    </{tag}>']
        out.append('  </link>')
        if link == "base_link_inertia":
            out.append('  <link name="base"/>')
    out += ['  <link name="flange"/>', '  <link name="tool0"/>']
    pi = num(math.pi)
    out += ['  <joint name="base_link-base_link_inertia" type="fixed">', '    <parent link="base_link"/>',
            '    <child link="base_link_inertia"/>', f'    <origin xyz="0 0 0" rpy="0 0 {pi}"/>', '  </joint>',
            '  <joint name="base_link-base_fixed_joint" type="fixed">', '    <parent link="base_link"/>',
            '    <child link="base"/>', f'    <origin xyz="0 0 0" rpy="0 0 {pi}"/>', '  </joint>']
    joints = [("shoulder_pan_joint", "base_link_inertia", "shoulder_link", "shoulder"),
              ("shoulder_lift_joint", "shoulder_link", "upper_arm_link", "upper_arm"),
              ("elbow_joint", "upper_arm_link", "forearm_link", "forearm"),
              ("wrist_1_joint", "forearm_link", "wrist_1_link", "wrist_1"),
              ("wrist_2_joint", "wrist_1_link", "wrist_2_link", "wrist_2"),
              ("wrist_3_joint", "wrist_2_link", "wrist_3_link", "wrist_3")]
    for name, parent, child, part in joints:
        l = lim[name]
        # A joint without position limits (the UR3e's last wrist) gets the controller's +-360 degrees.
        lower = l.get("min_position", -2 * math.pi) if l.get("has_position_limits", True) else -2 * math.pi
        upper = l.get("max_position", 2 * math.pi) if l.get("has_position_limits", True) else 2 * math.pi
        out += [f'  <joint name="{name}" type="revolute">', f'    <parent link="{parent}"/>', f'    <child link="{child}"/>',
                f'    <origin xyz="{xyz(kin[part])}" rpy="{rpy(kin[part])}"/>', '    <axis xyz="0 0 1"/>',
                f'    <limit lower="{num(lower)}" upper="{num(upper)}" velocity="{num(l["max_velocity"])}" effort="{num(l["max_effort"])}"/>',
                '  </joint>']
    h = num(math.pi / 2)
    out += ['  <joint name="wrist_3-flange" type="fixed">', '    <parent link="wrist_3_link"/>', '    <child link="flange"/>',
            f'    <origin xyz="0 0 0" rpy="0 -{h} -{h}"/>', '  </joint>',
            '  <joint name="flange-tool0" type="fixed">', '    <parent link="flange"/>', '    <child link="tool0"/>',
            f'    <origin xyz="0 0 0" rpy="{h} 0 {h}"/>', '  </joint>', '</robot>', '']
    sys.stdout.write("\n".join(out))

main(sys.argv[1], sys.argv[2], sys.argv[3] == "visual")
