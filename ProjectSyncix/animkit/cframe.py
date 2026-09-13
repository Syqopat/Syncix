"""Roblox CFrame arithmetic in plain Python (no numpy), enough to pose a rig.

A CFrame is a position plus a 3x3 rotation, stored row-major as Roblox writes it in XML
(R00 R01 R02 / R10 R11 R12 / R20 R21 R22). Columns are the frame's right, up and back
vectors, as in Roblox: `look` (forward) is minus the third column.

How a Motor6D places its child (Roblox's rule, used for the preview):
    Part1.CFrame = Part0.CFrame * C0 * Transform * C1:Inverse()
A Pose in a KeyframeSequence is that Transform for the joint whose Part1 it is named after.
"""
import math

IDENTITY_ROT = (1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)


class CFrame:
    __slots__ = ("p", "r")

    def __init__(self, p=(0.0, 0.0, 0.0), r=IDENTITY_ROT):
        self.p = tuple(float(v) for v in p)
        self.r = tuple(float(v) for v in r)

    # ------------------------------------------------------------------ builders
    @staticmethod
    def new(x=0.0, y=0.0, z=0.0):
        return CFrame((x, y, z))

    @staticmethod
    def from_components(values):
        """The 12 numbers Roblox writes: x, y, z, R00 .. R22."""
        v = [float(c) for c in values]
        return CFrame(v[0:3], v[3:12])

    @staticmethod
    def angles(rx, ry, rz):
        """Radians, like Roblox's CFrame.Angles / fromEulerAnglesXYZ: R = Rx * Ry * Rz."""
        return rot_x(rx) * rot_y(ry) * rot_z(rz)

    @staticmethod
    def degrees(rx=0.0, ry=0.0, rz=0.0):
        return CFrame.angles(math.radians(rx), math.radians(ry), math.radians(rz))

    # ------------------------------------------------------------------ algebra
    def __mul__(self, other):
        a, b = self.r, other.r
        r = (
            a[0] * b[0] + a[1] * b[3] + a[2] * b[6], a[0] * b[1] + a[1] * b[4] + a[2] * b[7], a[0] * b[2] + a[1] * b[5] + a[2] * b[8],
            a[3] * b[0] + a[4] * b[3] + a[5] * b[6], a[3] * b[1] + a[4] * b[4] + a[5] * b[7], a[3] * b[2] + a[4] * b[5] + a[5] * b[8],
            a[6] * b[0] + a[7] * b[3] + a[8] * b[6], a[6] * b[1] + a[7] * b[4] + a[8] * b[7], a[6] * b[2] + a[7] * b[5] + a[8] * b[8],
        )
        return CFrame(self.point(other.p), r)

    def point(self, v):
        """A point in this frame's local space, in world space."""
        a, p = self.r, self.p
        return (
            a[0] * v[0] + a[1] * v[1] + a[2] * v[2] + p[0],
            a[3] * v[0] + a[4] * v[1] + a[5] * v[2] + p[1],
            a[6] * v[0] + a[7] * v[1] + a[8] * v[2] + p[2],
        )

    def vector(self, v):
        """A direction in local space, in world space (rotation only)."""
        a = self.r
        return (
            a[0] * v[0] + a[1] * v[1] + a[2] * v[2],
            a[3] * v[0] + a[4] * v[1] + a[5] * v[2],
            a[6] * v[0] + a[7] * v[1] + a[8] * v[2],
        )

    def inverse(self):
        a = self.r
        t = (a[0], a[3], a[6], a[1], a[4], a[7], a[2], a[5], a[8])  # transpose
        p = self.p
        inv_p = (
            -(t[0] * p[0] + t[1] * p[1] + t[2] * p[2]),
            -(t[3] * p[0] + t[4] * p[1] + t[5] * p[2]),
            -(t[6] * p[0] + t[7] * p[1] + t[8] * p[2]),
        )
        return CFrame(inv_p, t)

    def rotation(self):
        return CFrame((0.0, 0.0, 0.0), self.r)

    def translated(self, dx, dy, dz):
        return CFrame((self.p[0] + dx, self.p[1] + dy, self.p[2] + dz), self.r)

    def components(self):
        return self.p + self.r

    def to_euler_degrees(self):
        """(rx, ry, rz) such that CFrame.degrees(rx, ry, rz) has this rotation."""
        r = self.r
        ry = math.asin(max(-1.0, min(1.0, r[2])))
        if abs(r[2]) < 0.99999:
            rx = math.atan2(-r[5], r[8])
            rz = math.atan2(-r[1], r[0])
        else:  # gimbal lock
            rx = math.atan2(r[7], r[4])
            rz = 0.0
        return tuple(math.degrees(v) for v in (rx, ry, rz))

    def is_close(self, other, eps=1e-4):
        return all(abs(a - b) < eps for a, b in zip(self.components(), other.components()))

    def __repr__(self):
        return "CFrame(" + ", ".join(f"{v:.4g}" for v in self.components()) + ")"


def rot_x(a):
    c, s = math.cos(a), math.sin(a)
    return CFrame(r=(1, 0, 0, 0, c, -s, 0, s, c))


def rot_y(a):
    c, s = math.cos(a), math.sin(a)
    return CFrame(r=(c, 0, s, 0, 1, 0, -s, 0, c))


def rot_z(a):
    c, s = math.cos(a), math.sin(a)
    return CFrame(r=(c, -s, 0, s, c, 0, 0, 0, 1))


def axis_angle(axis, a):
    """Rotation of `a` radians about a unit axis (Rodrigues)."""
    x, y, z = axis
    n = math.sqrt(x * x + y * y + z * z) or 1.0
    x, y, z = x / n, y / n, z / n
    c, s, t = math.cos(a), math.sin(a), 1 - math.cos(a)
    return CFrame(r=(
        t * x * x + c, t * x * y - s * z, t * x * z + s * y,
        t * x * y + s * z, t * y * y + c, t * y * z - s * x,
        t * x * z - s * y, t * y * z + s * x, t * z * z + c,
    ))


def lerp(a, b, t):
    """Position linear, rotation by slerp through axis-angle of a^-1 * b."""
    rel = a.rotation().inverse() * b.rotation()
    r = rel.r
    angle = math.acos(max(-1.0, min(1.0, (r[0] + r[4] + r[8] - 1) / 2)))
    if angle < 1e-6:
        rot = a.rotation()
    else:
        axis = (r[7] - r[5], r[2] - r[6], r[3] - r[1])
        rot = a.rotation() * axis_angle(axis, angle * t)
    p = tuple(pa + (pb - pa) * t for pa, pb in zip(a.p, b.p))
    return CFrame(p, rot.r)
