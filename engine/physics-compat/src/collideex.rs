// Ported from Visual Pinball X: src/physics/collideex.h and src/physics/collideex.cpp (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436), with FIX_PHYSICS and NEW_PHYSICS off.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's 3D colliders: flat polygons (wall tops, triggers), triangles (ramps,
//! rubbers, 3D objects), the playfield plane and 3D line segments; and the
//! part colliders of the same VPX file: bumpers, slingshots, gates, spinners
//! and triggers.

use crate::collide::{CollisionEvent, HitCircle, HitLineZ, HitObjectBase, LineSeg, ObjType};
use crate::env::{PartId, PhysicsEnv, PhysicsEvent};
use crate::hitball::{BallS, HitBall};
use crate::math::def::{ang_to_rad, clamp, inf_nan, max, min};
use crate::math::{Matrix3, Vertex2D, Vertex3Ds, cross_product};
use crate::physconst::{C_CONTACTVEL, C_LOWNORMVEL, PHYS_FACTOR, PHYS_SKIN, PHYS_TOUCH, STATICTIME};

/// The hit-event rule VPX's 3D colliders share (`Hit3DPoly`, `HitTriangle`,
/// `HitLine3D`): primitives (also ramps and rubbers) and standing hit targets
/// report the impact speed, then a hit event.
fn fire_3d_hit_event(base: &HitObjectBase, dot: f32, ball: &mut HitBall, env: &mut PhysicsEnv) {
    let Some(obj) = base.obj else { return };
    if base.fe && dot >= base.threshold {
        if base.obj_type == ObjType::Primitive {
            // FIXME Primitive but also Ramp or Rubbers
            env.events.push(PhysicsEvent::HitThreshold { part: obj, value: dot });
            base.fire_hit_event(ball, env);
        } else if base.obj_type == ObjType::HitTarget && !env.dropped_targets.contains(&obj) {
            env.events.push(PhysicsEvent::TargetHit { part: obj });
            env.events.push(PhysicsEvent::HitThreshold { part: obj, value: dot });
            base.fire_hit_event(ball, env);
        }
    } else if base.obj_type == ObjType::Primitive || (base.obj_type == ObjType::HitTarget && !env.dropped_targets.contains(&obj)) {
        // a part the table has its hits off for, where achievements watch it: a standing
        // target's while it stands
        base.watched(dot, ball, env);
    }
}

/// VPX's `Hit3DPoly`: a flat convex or concave polygon in 3D.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit3DPoly {
    pub rgv: Vec<Vertex3Ds>,
    pub normal: Vertex3Ds,
}

impl Hit3DPoly {
    /// VPX's constructor and `Init`: Newell's normal, flipped, so vertices
    /// must be counterclockwise. Also sets the base's material values.
    pub fn new(rgv: Vec<Vertex3Ds>, base: &mut HitObjectBase) -> Self {
        let mut normal = Vertex3Ds::new(0.0, 0.0, 0.0);

        // Newell's method for normal computation
        let n = rgv.len();
        for i in 0..n {
            let m = if i < n - 1 { i + 1 } else { 0 };

            normal.x += (rgv[i].y - rgv[m].y) * (rgv[i].z + rgv[m].z);
            normal.y += (rgv[i].z - rgv[m].z) * (rgv[i].x + rgv[m].x);
            normal.z += (rgv[i].x - rgv[m].x) * (rgv[i].y + rgv[m].y);
        }

        let sqr_len = normal.x * normal.x + normal.y * normal.y + normal.z * normal.z;
        let inv_len = if sqr_len > 0.0 { -1.0 / sqr_len.sqrt() } else { 0.0 }; // NOTE: normal is flipped! Thus we need vertices in CCW order
        normal.x *= inv_len;
        normal.y *= inv_len;
        normal.z *= inv_len;

        base.elasticity = 0.3;
        base.friction = 0.3;
        base.scatter = 0.0;

        Self { rgv, normal }
    }

    /// VPX's circular constructor: a regular polygon of `sections` sides.
    pub fn new_circle(x: f32, y: f32, z: f32, r: f32, sections: usize, base: &mut HitObjectBase) -> Self {
        let inv_sections = (std::f64::consts::PI * 2.0) as f32 / sections as f32;

        let rgv = (0..sections)
            .map(|i| {
                let angle = inv_sections * i as f32;
                Vertex3Ds::new(x + angle.sin() * r, y + angle.cos() * r, z)
            })
            .collect();

        Self::new(rgv, base)
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        let b = &mut base.hit_bbox;
        b.left = self.rgv[0].x;
        b.right = self.rgv[0].x;
        b.top = self.rgv[0].y;
        b.bottom = self.rgv[0].y;
        b.zlow = self.rgv[0].z;
        b.zhigh = self.rgv[0].z;

        for v in &self.rgv[1..] {
            b.left = min(v.x, b.left);
            b.right = max(v.x, b.right);
            b.top = min(v.y, b.top);
            b.bottom = max(v.y, b.bottom);
            b.zlow = min(v.z, b.zlow);
            b.zhigh = max(v.z, b.zhigh);
        }
    }

    /// VPX's `Hit3DPoly::HitTest`. Rigid unless the base is a trigger.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        let bnv = self.normal.dot(&ball.vel); // speed in Normal-vector direction

        if (base.obj_type != ObjType::Trigger) && (bnv > C_LOWNORMVEL) {
            // return if clearly ball is receding from object
            return -1.0;
        }

        // Point on the ball that will hit the polygon, if it hits at all
        let mut hit_pos = ball.pos - ball.radius * self.normal; // nearest point on ball ... projected radius along norm

        let bnd = self.normal.dot(&(hit_pos - self.rgv[0])); // distance from plane to ball

        let mut b_unhit = bnv > C_LOWNORMVEL;
        let inside = bnd <= 0.0; // in ball inside object volume

        let rigid = base.obj_type != ObjType::Trigger;
        let hittime: f32;
        if rigid {
            // rigid polygon
            if bnd < -ball.radius {
                return -1.0; // (ball normal distance) excessive penetration of object skin ... no collision HACK
            }

            if bnd <= PHYS_TOUCH as f32 {
                if inside || (bnv.abs() > C_CONTACTVEL) // fast velocity, return zero time
                    //zero time for rigid fast bodies
                    || (bnd <= (-PHYS_TOUCH) as f32)
                {
                    // slow moving but embedded
                    hittime = 0.0;
                } else {
                    hittime = bnd * (1.0 / (2.0 * PHYS_TOUCH)) as f32 + 0.5; // don't compete for fast zero time events
                }
            } else if bnv.abs() > C_LOWNORMVEL {
                // not velocity low?
                hittime = bnd / -bnv; // rate ok for safe divide
            } else {
                return -1.0; // wait for touching
            }
        } else {
            // non-rigid polygon
            if bnv * bnd >= 0.0 {
                // outside-receding || inside-approaching
                if ball.vp_vol_objs.is_none() // temporary ball
                    // if trigger, then check:
                    || (bnd.abs() >= ball.radius * 0.5) // not too close ... nor too far away
                    || (inside != !ball.is_inside(base.obj))
                {
                    // ...ball outside and hit set or ball inside and no hit set
                    return -1.0;
                }

                hittime = 0.0;
                b_unhit = !inside; // ball on outside is UnHit, otherwise it's a Hit
            } else {
                hittime = bnd / -bnv;
            }
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // time is outside this frame ... no collision
        }

        hit_pos += hittime * ball.vel; // advance hit point to contact

        // Do a point in poly test, using the xy plane, to see if the hit point is inside the polygon
        let n = self.rgv.len();
        let mut x2 = self.rgv[0].x;
        let mut y2 = self.rgv[0].y;
        let mut hx2 = hit_pos.x >= x2;
        let mut hy2 = hit_pos.y <= y2;
        let mut crosscount = 0; // count of lines which the hit point is to the left of
        for i in 0..n {
            let x1 = x2;
            let y1 = y2;
            let hx1 = hx2;
            let hy1 = hy2;

            let j = if i < n - 1 { i + 1 } else { 0 };
            x2 = self.rgv[j].x;
            y2 = self.rgv[j].y;
            hx2 = hit_pos.x >= x2;
            hy2 = hit_pos.y <= y2;

            if (y1 == y2) ||
                (hy1 && hy2) || (!hy1 && !hy2) || // if out of y range, forget about this segment
                (hx1 && hx2)
            {
                // Hit point is on the right of the line
                continue;
            }

            if !hx1 && !hx2 {
                crosscount ^= 1;
                continue;
            }

            if x2 == x1 {
                if !hx2 {
                    crosscount ^= 1;
                }
                continue;
            }

            // Now the hard part - the hit point is in the line bounding box
            if x2 - (y2 - hit_pos.y) * (x1 - x2) / (y1 - y2) > hit_pos.x {
                crosscount ^= 1;
            }
        }

        if crosscount & 1 != 0 {
            coll.hitnormal = self.normal;

            if !rigid {
                // non rigid body collision? return direction
                coll.hitflag = b_unhit; // UnHit signal -> is receding from outside target
            }

            coll.hitdistance = bnd; // 3dhit actual contact distance ...

            return hittime;
        }

        -1.0
    }

    /// VPX's `Hit3DPoly::Collide`: a bounce, or for a trigger, entering or leaving it.
    pub fn collide(&self, base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let hitnormal = coll.hitnormal;

        if base.obj_type != ObjType::Trigger {
            let dot = -(hitnormal.dot(&ball.d.vel));

            ball.collide_3d_wall(&self.normal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

            fire_3d_hit_event(base, dot, ball, env);
        } else {
            // trigger:
            let Some(set) = ball.d.vp_vol_objs.clone() else { return };

            let i = base.obj.and_then(|obj| set.borrow().iter().position(|p| *p == obj)); // if None then not in objects volume set (i.e not already hit)

            if (!coll.hitflag) == i.is_none() {
                // Hit == NotAlreadyHit
                ball.d.pos += STATICTIME * ball.d.vel; // move ball slightly forward

                match i {
                    None => {
                        if let Some(obj) = base.obj {
                            set.borrow_mut().push(obj);
                            env.events.push(PhysicsEvent::Hit { part: obj });
                        }
                    }
                    Some(index) => {
                        set.borrow_mut().remove(index);
                        if let Some(obj) = base.obj {
                            env.events.push(PhysicsEvent::Unhit { part: obj });
                        }
                    }
                }
            }
        }
    }
}

/// VPX's `HitTriangle`. Only the face: the edges and corners of a mesh are
/// separate colliders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTriangle {
    pub rgv: [Vertex3Ds; 3],
    pub normal: Vertex3Ds,
}

impl HitTriangle {
    /// VPX's constructor, vertices counterclockwise. Thin triangles get a zero
    /// normal and count as degenerate. Also sets the base's material values.
    pub fn new(rgv: [Vertex3Ds; 3], base: &mut HitObjectBase) -> Self {
        // due to the swapping of the order of e0 and e1,
        // the vertices must be passed in counterclockwise order
        // (but rendering uses clockwise order!)
        let e0 = rgv[2] - rgv[0];
        let e1 = rgv[1] - rgv[0];
        let mut normal = cross_product(&e0, &e1);
        // FIXME We have a precision issue were thin triangle can lead to wrong hit so we mark as degenerate if precision is too low.
        let ls = normal.length_squared();
        if ls < 0.01 {
            normal.set_zero();
        } else {
            normal /= ls.sqrt();
        }

        base.elasticity = 0.3;
        base.friction = 0.3;
        base.scatter = 0.0;

        Self { rgv, normal }
    }

    pub fn is_degenerate(&self) -> bool {
        self.normal.is_zero()
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        let r = &self.rgv;
        base.hit_bbox.left = min(r[0].x, min(r[1].x, r[2].x));
        base.hit_bbox.right = max(r[0].x, max(r[1].x, r[2].x));
        base.hit_bbox.top = min(r[0].y, min(r[1].y, r[2].y));
        base.hit_bbox.bottom = max(r[0].y, max(r[1].y, r[2].y));
        base.hit_bbox.zlow = min(r[0].z, min(r[1].z, r[2].z));
        base.hit_bbox.zhigh = max(r[0].z, max(r[1].z, r[2].z));
    }

    /// VPX's `HitTriangle::HitTest`.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        debug_assert!(!self.is_degenerate());
        if !base.enabled {
            return -1.0;
        }

        let bnv = self.normal.dot(&ball.vel); // speed in Normal-vector direction

        if bnv > C_CONTACTVEL {
            // return if clearly ball is receding from object
            return -1.0;
        }

        // Point on the ball that will hit the polygon, if it hits at all
        let mut hit_pos = ball.pos - ball.radius * self.normal; // nearest point on ball ... projected radius along norm

        let bnd = self.normal.dot(&(hit_pos - self.rgv[0])); // distance from plane to ball

        if bnd < -ball.radius {
            return -1.0; // (ball normal distance) excessive penetration of object skin ... no collision HACK
        }

        let mut is_contact = false;
        let hittime: f32;

        if bnd <= PHYS_TOUCH as f32 {
            if bnv.abs() <= C_CONTACTVEL {
                hittime = 0.0;
                is_contact = true;
            } else if bnd <= 0.0 {
                hittime = 0.0; // zero time for rigid fast bodies
            } else {
                hittime = bnd / -bnv;
            }
        } else if bnv.abs() > C_LOWNORMVEL {
            // not velocity low?
            hittime = bnd / -bnv; // rate ok for safe divide
        } else {
            return -1.0; // wait for touching
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // time is outside this frame ... no collision
        }

        hit_pos += hittime * ball.vel; // advance hit point to contact

        // check if hitPos is within the triangle

        // Compute vectors
        let v0 = self.rgv[2] - self.rgv[0];
        let v1 = self.rgv[1] - self.rgv[0];
        let v2 = hit_pos - self.rgv[0];

        // Compute dot products
        let dot00 = v0.dot(&v0);
        let dot01 = v0.dot(&v1);
        let dot02 = v0.dot(&v2);
        let dot11 = v1.dot(&v1);
        let dot12 = v1.dot(&v2);

        // Compute barycentric coordinates
        let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

        // Check if point is in triangle
        if (u >= 0.0) && (v >= 0.0) && (u + v <= 1.0) {
            coll.hitnormal = self.normal;

            coll.hitdistance = bnd; // 3dhit actual contact distance ...

            coll.is_contact = is_contact;
            if is_contact {
                coll.hit_org_normalvelocity = bnv;
            }

            hittime
        } else {
            -1.0
        }
    }

    /// VPX's `HitTriangle::Collide`.
    pub fn collide(&self, base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = -(coll.hitnormal.dot(&ball.d.vel));

        ball.collide_3d_wall(&self.normal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        fire_3d_hit_event(base, dot, ball, env);
    }
}

/// VPX's `HitPlane`: an infinite plane (the playfield, the glass). Not in the
/// collision search, as it has no bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitPlane {
    pub normal: Vertex3Ds,
    pub d: f32,
}

impl HitPlane {
    pub fn new(normal: Vertex3Ds, d: f32) -> Self {
        Self { normal, d }
    }

    /// VPX's `HitPlane::HitTest` (the version without `NEW_PHYSICS`).
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        let bnv = self.normal.dot(&ball.vel); // speed in normal direction

        if bnv > C_CONTACTVEL {
            // return if clearly ball is receding from object
            return -1.0;
        }

        let bnd = self.normal.dot(&ball.pos) - ball.radius - self.d; // distance from plane to ball surface

        if bnd < ball.radius * -2.0 {
            // !! solely responsible for ball through playfield?? check other places, too (radius*2??)
            return -1.0; // excessive penetration of plane ... no collision HACK
        }

        if bnv.abs() <= C_CONTACTVEL {
            if bnd.abs() <= PHYS_TOUCH as f32 {
                coll.is_contact = true;
                coll.hitnormal = self.normal;
                coll.hit_org_normalvelocity = bnv; // remember original normal velocity
                coll.hitdistance = bnd;
                return 0.0; // hittime is ignored for contacts
            } else {
                return -1.0; // large distance, small velocity -> no hit
            }
        }

        let mut hittime = bnd / -bnv; // rate ok for safe divide
        if hittime < 0.0 {
            hittime = 0.0; // already penetrating? then collide immediately
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // time is outside this frame ... no collision
        }

        coll.hitnormal = self.normal;
        coll.hitdistance = bnd; // actual contact distance

        hittime
    }

    /// VPX's `HitPlane::Collide`, with `C_EMBEDSHOT_PLANE` (enabled in VPX):
    /// a ball that has sunk into the plane is pushed back out.
    pub fn collide(&self, base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        // if ball has penetrated, push it out of the plane
        let bnd = self.normal.dot(&ball.d.pos) - ball.d.radius - self.d; // distance from plane to ball surface
        if bnd < 0.0 {
            ball.d.pos -= bnd * self.normal;
        }
    }
}

/// VPX's `HitLine3D`: a line segment in any direction (ramp and rubber edges),
/// handled as a vertical line in a rotated frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitLine3D {
    pub linez: HitLineZ,
    pub matrix: Matrix3,
    pub v1: Vertex3Ds,
    pub v2: Vertex3Ds,
}

impl HitLine3D {
    /// VPX's constructor; it also sets the base's bounding box.
    pub fn new(v1: Vertex3Ds, v2: Vertex3Ds, base: &mut HitObjectBase) -> Self {
        let mut v_line = v2 - v1;
        v_line.normalize();

        // Axis of rotation to make 3D cylinder a cylinder along the z-axis
        let mut transaxis = Vertex3Ds::new(v_line.y, -v_line.x, 0.0);

        let l = transaxis.length_squared();
        if l <= 1e-6 {
            // line already points in z axis?
            transaxis.set(1.0, 0.0, 0.0); // choose arbitrary rotation vector
        } else {
            transaxis /= l.sqrt();
        }

        // Angle to rotate the line into the z-axis
        let dot = v_line.z;

        let mut matrix = Matrix3::default();
        matrix.rotation_around_axis_sin_cos(&transaxis, -(1.0 - dot * dot).sqrt(), dot);

        let vtrans1 = matrix * v1;
        let vtrans2z = (matrix * v2).z;

        // set up HitLineZ parameters
        let linez = HitLineZ::new(crate::math::Vertex2D::new(vtrans1.x, vtrans1.y), min(vtrans1.z, vtrans2z), max(vtrans1.z, vtrans2z));

        base.hit_bbox.left = min(v1.x, v2.x);
        base.hit_bbox.right = max(v1.x, v2.x);
        base.hit_bbox.top = min(v1.y, v2.y);
        base.hit_bbox.bottom = max(v1.y, v2.y);
        base.hit_bbox.zlow = min(v1.z, v2.z);
        base.hit_bbox.zhigh = max(v1.z, v2.z);

        Self { linez, matrix, v1, v2 }
    }

    /// VPX's `HitLine3D::HitTest`: rotate the ball into the line's frame,
    /// test against a vertical line, rotate the normal back.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        let mut ball_tmp = ball.clone();
        // transform ball to cylinder coordinate system
        ball_tmp.pos = self.matrix * ball.pos;
        ball_tmp.vel = self.matrix * ball.vel;

        let hittime = self.linez.hit_test(base, &ball_tmp, dtime, coll);

        if hittime >= 0.0 {
            // transform hit normal back to world coordinate system
            coll.hitnormal = self.matrix.mul_vector_t(&coll.hitnormal);
        }

        hittime
    }

    /// VPX's `HitLine3D::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = -(coll.hitnormal.dot(&ball.d.vel));

        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        fire_3d_hit_event(base, dot, ball, env);
    }
}

// ---------------------------------------------------------------------------------------------
// The part colliders of VPX's collideex.cpp: bumpers, slingshots, gates, spinners, triggers.
// Each holds a copy of the part settings VPX reads from the part (`m_d`) during play; the engine
// updates the copies when a script changes a setting.
// ---------------------------------------------------------------------------------------------

/// The bumper part's settings VPX reads during a collision (`Bumper::m_d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BumperData {
    /// Does a hit kick the ball and fire the hit event? (`m_hitEvent`)
    pub hit_event: bool,
    /// Minimum impact speed for the kick.
    pub threshold: f32,
    /// The kick's speed.
    pub force: f32,
}

/// VPX's `BumperHitCircle`: the bumper's body, which kicks the ball away when
/// hit hard enough.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BumperHitCircle {
    pub circle: HitCircle,
    pub part: PartId,
    pub data: BumperData,
    /// For the ring and skirt animation: set on a kick, read by the renderer.
    pub anim_hit_event: bool,
    pub anim_hit_ball_position: Vertex3Ds,
    pub anim_ring_anim_offset: f32,
}

impl BumperHitCircle {
    /// VPX's constructor. `base` comes from `HitObjectBase::with_z(zlow, zhigh)`.
    pub fn new(part: PartId, data: BumperData, center: Vertex2D, radius: f32) -> Self {
        Self {
            circle: HitCircle::new(center, radius),
            part,
            data,
            anim_hit_event: true,
            anim_hit_ball_position: Vertex3Ds::default(), // VPX leaves it unset until the first kick
            anim_ring_anim_offset: 0.0,
        }
    }

    /// VPX's `BumperHitCircle::Collide`: a bounce, then the kick.
    pub fn collide(&mut self, base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        if !base.enabled {
            return;
        }

        let dot = coll.hitnormal.dot(&ball.d.vel); // needs to be computed before Collide3DWall()!

        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env); // reflect ball from wall

        if self.data.hit_event && dot <= -self.data.threshold {
            // if velocity greater than threshold level
            ball.d.vel += coll.hitnormal * self.data.force; // add a chunk of velocity to drive ball away

            self.anim_hit_event = true;
            self.anim_hit_ball_position = ball.d.pos;
            env.events.push(PhysicsEvent::Hit { part: self.part });
        } else {
            // a bumper the table has its hits off for (it doesn't kick), where achievements watch it
            base.watched(-dot, ball, env);
        }
    }
}

/// The wall part's live settings its slingshot segments read (`Surface`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlingshotData {
    /// Minimum impact speed (`m_d.m_slingshot_threshold`).
    pub threshold: f32,
    /// `m_disabled`: the slingshot doesn't fire.
    pub disabled: bool,
}

/// VPX's `LineSegSlingshot`: a wall segment that kicks the ball back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegSlingshot {
    pub seg: LineSeg,
    /// The wall part.
    pub part: PartId,
    pub data: SlingshotData,
    /// The kick's strength (`m_force`, the wall's slingshot force).
    pub force: f32,
    /// When the slingshot animation resets, in ms; 0 while at rest.
    pub time_reset: u32,
}

impl LineSegSlingshot {
    pub fn new(part: PartId, data: SlingshotData, force: f32, p1: Vertex2D, p2: Vertex2D) -> Self {
        Self { seg: LineSeg::new(p1, p2), part, data, force, time_reset: 0 }
    }

    /// VPX's `LineSegSlingshot::HitTest`: normal face, lateral, rigid.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        self.seg.hit_test_basic(base, ball, dtime, coll, true, true, true)
    }

    /// VPX's `LineSegSlingshot::Collide`: the kick, strongest at the middle of
    /// the segment, then a bounce, then the slingshot event.
    pub fn collide(&mut self, base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
        let hitnormal = coll.hitnormal;

        let dot = coll.hitnormal.dot(&pball.d.vel); // normal velocity to slingshot

        let threshold = dot <= -self.data.threshold; // normal greater than threshold?

        if !self.data.disabled && threshold {
            // enabled and if velocity greater than threshold level
            let (v1, v2) = (self.seg.v1, self.seg.v2);
            let len = (v2.x - v1.x) * hitnormal.y - (v2.y - v1.y) * hitnormal.x; // length of segment, Unit TAN points from V1 to V2

            let vhitpoint = Vertex2D::new(
                pball.d.pos.x - hitnormal.x * pball.d.radius, // project ball radius along norm
                pball.d.pos.y - hitnormal.y * pball.d.radius,
            );

            // vhitpoint will now be the point where the ball hits the line
            // Calculate this distance from the center of the slingshot to get force

            let btd = (vhitpoint.x - v1.x) * hitnormal.y - (vhitpoint.y - v1.y) * hitnormal.x; // distance to vhit from V1
            let mut force = if len.abs() > 1.0e-6 { (btd + btd) / len - 1.0 } else { -1.0 }; // -1..+1
            force = 0.5 * (1.0 - force * force); // !! maximum value 0.5 ...I think this should have been 1.0...oh well
            // will match the previous physics
            force *= self.force; // -80;

            pball.d.vel -= hitnormal * force; // boost velocity, drive into slingshot (counter normal), allow CollideWall to handle the remainder
        }

        pball.collide_3d_wall(&hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        if base.obj.is_some() && base.fe && !self.data.disabled && threshold {
            // is this the same place as last event? if same then ignore it
            let dist_ls = (pball.last_event_pos - pball.d.pos).length_squared();
            pball.last_event_pos = pball.d.pos; // remember last collide position

            if dist_ls > 0.25 {
                // !! magic distance, must be a new place if only by a little
                env.events.push(PhysicsEvent::Slingshot { part: self.part });
                self.time_reset = env.time_msec.wrapping_add(100);
                // and the controller rumbles
                env.rumble = Some((0.15, 0.1, 100));
            }
        }
    }
}

/// The gate part's live settings its collider reads (`Gate::m_d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateData {
    /// `m_twoWay`, as the mover reads it.
    pub two_way: bool,
    /// The gate's height above its surface (`m_height`).
    pub height: f32,
}

/// VPX's `GateMoverObject`: the gate flap's swing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateMover {
    pub anglespeed: f32,
    pub angle: f32,
    pub damping: f32,
    pub gravityfactor: f32,
    /// Open and close angles when swinging freely; animation limits when moved by the script.
    pub angle_min: f32,
    pub angle_max: f32,
    /// The script holds the gate open (not just a ball passing through).
    pub open: bool,
    /// The script is opening or closing the gate.
    pub forced_move: bool,
    /// For the direction of the little bounce-back.
    pub hit_direction: bool,
}

impl GateMover {
    /// VPX's `GateMoverObject::UpdateVelocities`: gravity pulls the flap back
    /// down, unless the script holds it open.
    pub fn update_velocities(&mut self) {
        if !self.open {
            if self.angle.abs() < (self.angle_min + 0.01) && self.anglespeed.abs() < 0.01 {
                // stop a bit earlier to prevent a nearly endless animation (especially for slow balls)
                self.angle = self.angle_min;
                self.anglespeed = 0.0;
            }
            if self.anglespeed.abs() != 0.0 && self.angle != self.angle_min {
                // Center of gravity towards bottom of object, makes it stop vertical
                self.anglespeed -= self.angle.sin() * self.gravityfactor * (PHYS_FACTOR / 100.0) as f32;
                self.anglespeed *= self.damping;
            }
        }
    }
}

/// VPX's `HitGate`: two one-sided segments, one per face, which swing the
/// flap instead of bouncing the ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitGate {
    pub part: PartId,
    pub data: GateData,
    pub mover: GateMover,
    /// `m_twoWay`, as the collision reads it.
    pub two_way: bool,
    pub segs: [LineSeg; 2],
    /// The segments' own hit fields (their height range).
    pub seg_hit: [HitObjectBase; 2],
}

impl HitGate {
    /// VPX's `HitGate` constructor. Angles in radians (VPX stores a gate's
    /// angles in radians, with any inversion already corrected); `height` is
    /// the surface the gate stands on.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        part: PartId,
        data: GateData,
        center: Vertex2D,
        length: f32,
        rotation_deg: f32,
        height: f32,
        angle_min: f32,
        angle_max: f32,
        damping: f32,
        gravityfactor: f32,
    ) -> Self {
        let halflength = length * 0.5;

        let radangle = ang_to_rad(rotation_deg);
        let sn = radangle.sin();
        let cs = radangle.cos();

        let mut seg_hit = [HitObjectBase::with_z(height, height + (2.0 * PHYS_SKIN) as f32); 2];
        seg_hit[0].obj_type = ObjType::Gate;
        seg_hit[1].obj_type = ObjType::Gate;

        let skin = PHYS_SKIN as f32;
        let mut segs = [LineSeg::default(); 2];
        segs[0].v2.x = center.x + cs * (halflength + skin); // oversize by the ball radius
        segs[0].v2.y = center.y + sn * (halflength + skin); // this will prevent clipping
        segs[0].v1.x = center.x - cs * (halflength + skin); // through the edge of the
        segs[0].v1.y = center.y - sn * (halflength + skin); // spinner

        segs[1].v1.x = segs[0].v2.x;
        segs[1].v1.y = segs[0].v2.y;
        segs[1].v2.x = segs[0].v1.x;
        segs[1].v2.y = segs[0].v1.y;

        segs[0].calc_normal_and_length();
        segs[1].calc_normal_and_length();

        let mover = GateMover {
            angle_min,
            angle_max,
            angle: angle_min,
            anglespeed: 0.0,
            damping: damping.powf(PHYS_FACTOR as f32), // 0.996f;
            gravityfactor,
            open: false,
            forced_move: false,
            hit_direction: false,
        };
        Self { part, data, mover, two_way: false, segs, seg_hit }
    }

    /// VPX's `HitGate::HitTest`: either face; `hitflag` tells which.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        for i in 0..2 {
            let hittime = self.segs[i].hit_test_basic(&self.seg_hit[i], ball, dtime, coll, false, true, false); // any face, lateral, non-rigid
            if hittime >= 0.0 {
                // signal the Collide() function that the hit is on the front or back side
                coll.hitflag = i != 0;

                return hittime;
            }
        }

        -1.0
    }

    /// VPX's `HitGate::Collide`: the ball swings the flap.
    pub fn collide(&mut self, base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = coll.hitnormal.dot(&ball.d.vel);
        let h = self.data.height * 0.5;

        // linear speed = ball speed
        // angular speed = linear/radius (height of hit)
        let mut speed = dot.abs();
        // h is the height of the gate axis.
        if h.abs() > 1.0 {
            // avoid divide by zero
            speed /= h;
        }

        self.mover.anglespeed = speed;
        if !coll.hitflag && !self.two_way {
            self.mover.hit_direction = dot > 0.0;
            self.mover.anglespeed *= (1.0_f64 / 50.0) as f32; // Give a little bounce-back.
            return; // Hit from back doesn't count if not two-way
        }

        self.mover.hit_direction = false;

        // We encoded which side of the gate the ball hit
        if coll.hitflag && self.two_way {
            self.mover.anglespeed = -self.mover.anglespeed;
        }

        base.fire_hit_event(ball, env);
    }

    /// VPX's `HitGate::CalcHitBBox`: both segments share one box.
    pub fn calc_hit_bbox(&mut self, base: &mut HitObjectBase) {
        self.segs[0].calc_hit_bbox(&mut self.seg_hit[0]);
        base.hit_bbox = self.seg_hit[0].hit_bbox;
    }
}

/// The spinner part's live settings its collider reads (`Spinner::m_d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinnerData {
    /// The spinner's height above its surface (`m_height`).
    pub height: f32,
    /// The part's angle limits in degrees; equal limits mean it spins freely.
    pub angle_min: f32,
    pub angle_max: f32,
}

/// VPX's `SpinnerMoverObject`: the plate's rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinnerMover {
    pub anglespeed: f32,
    pub angle: f32,
    /// In radians.
    pub angle_max: f32,
    pub angle_min: f32,
    pub elasticity: f32,
    pub damping: f32,
    pub visible: bool,
}

impl SpinnerMover {
    /// VPX's `SpinnerMoverObject::UpdateVelocities`: gravity and damping.
    pub fn update_velocities(&mut self) {
        self.anglespeed -= self.angle.sin() * (0.0025 * PHYS_FACTOR) as f32; // Center of gravity towards bottom of object, makes it stop vertical
        self.anglespeed *= self.damping;
    }
}

/// VPX's `HitSpinner`: two segments, one per face, which set the plate spinning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitSpinner {
    pub part: PartId,
    pub data: SpinnerData,
    pub mover: SpinnerMover,
    pub segs: [LineSeg; 2],
    pub seg_hit: [HitObjectBase; 2],
}

impl HitSpinner {
    /// VPX's `HitSpinner` constructor. `data`'s angles must already have any
    /// inversion corrected, as `Spinner::PhysicSetup` does; `height` is the
    /// surface the spinner stands on.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        part: PartId,
        data: SpinnerData,
        center: Vertex2D,
        length: f32,
        rotation_deg: f32,
        height: f32,
        damping: f32,
        elasticity: f32,
        visible: bool,
    ) -> Self {
        let halflength = length * 0.5;

        let radangle = ang_to_rad(rotation_deg);
        let sn = radangle.sin();
        let cs = radangle.cos();

        let mut seg_hit = [HitObjectBase::with_z(height, height + (2.0 * PHYS_SKIN) as f32); 2];
        seg_hit[0].obj_type = ObjType::Spinner;
        seg_hit[1].obj_type = ObjType::Spinner;

        let skin = PHYS_SKIN as f32;
        let mut segs = [LineSeg::default(); 2];
        segs[0].v2.x = center.x + cs * (halflength + skin); // oversize by the ball radius
        segs[0].v2.y = center.y + sn * (halflength + skin); // this will prevent clipping
        segs[0].v1.x = center.x - cs * (halflength + skin); // through the edge of the
        segs[0].v1.y = center.y - sn * (halflength + skin); // spinner
        segs[1].v1.x = segs[0].v2.x;
        segs[1].v1.y = segs[0].v2.y;
        segs[1].v2.x = segs[0].v1.x;
        segs[1].v2.y = segs[0].v1.y;

        segs[0].calc_normal_and_length();
        segs[1].calc_normal_and_length();

        let angle_max = ang_to_rad(data.angle_max);
        let angle_min = ang_to_rad(data.angle_min);
        let mover = SpinnerMover {
            angle_max,
            angle_min,
            angle: clamp(0.0, angle_min, angle_max),
            anglespeed: 0.0,
            // compute proper damping factor for physics framerate
            damping: damping.powf(PHYS_FACTOR as f32),
            elasticity,
            visible,
        };
        Self { part, data, mover, segs, seg_hit }
    }

    /// VPX's `HitSpinner::HitTest`: either face; `hitflag` tells which.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        for i in 0..2 {
            let hittime = self.segs[i].hit_test_basic(&self.seg_hit[i], ball, dtime, coll, false, true, false); // any face, lateral, non-rigid
            if hittime >= 0.0 {
                // signal the Collide() function that the hit is on the front or back side
                coll.hitflag = i == 0;

                return hittime;
            }
        }

        -1.0
    }

    /// VPX's `HitSpinner::Collide`: the ball sets the plate spinning.
    pub fn collide(&mut self, coll: &CollisionEvent, ball: &HitBall) {
        let dot = coll.hitnormal.dot(&ball.d.vel);

        if dot < 0.0 {
            return; // hit from back doesn't count
        }

        let h = self.data.height * 0.5;
        // linear speed = ball speed
        // angular speed = linear/radius (height of hit)

        // h is the height of the spinner axis;
        // Since the spinner has no mass in our equation, the spot
        // h -coll.m_radius will be moving a at linear rate of
        // 'speed'.  We can calculate the angular speed from that.

        self.mover.anglespeed = dot.abs(); // use this until a better value comes along

        if h.abs() > 1.0 {
            // avoid divide by zero
            self.mover.anglespeed /= h;
        }
        self.mover.anglespeed *= self.mover.damping;

        // We encoded which side of the spinner the ball hit
        if coll.hitflag {
            self.mover.anglespeed = -self.mover.anglespeed;
        }
    }

    /// VPX's `HitSpinner::CalcHitBBox`: both segments share one box.
    pub fn calc_hit_bbox(&mut self, base: &mut HitObjectBase) {
        self.segs[0].calc_hit_bbox(&mut self.seg_hit[0]);
        base.hit_bbox = self.seg_hit[0].hit_bbox;
    }
}

/// The ball enters or leaves a trigger or kicker volume: VPX's shared body of
/// `TriggerLineSeg::Collide` and `TriggerHitCircle::Collide`.
fn trigger_collide(base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
    let Some(set) = pball.d.vp_vol_objs.clone() else { return };

    let i = base.obj.and_then(|obj| set.borrow().iter().position(|p| *p == obj)); // if None then not in objects volume set (i.e not already hit)

    if (!coll.hitflag) == i.is_none() {
        // Hit == NotAlreadyHit
        pball.d.pos += STATICTIME * pball.d.vel; // move ball slightly forward

        match i {
            None => {
                if let Some(obj) = base.obj {
                    set.borrow_mut().push(obj);
                    env.events.push(PhysicsEvent::Hit { part: obj }); // also starts the trigger's animation
                }
            }
            Some(index) => {
                set.borrow_mut().remove(index);
                if let Some(obj) = base.obj {
                    env.events.push(PhysicsEvent::Unhit { part: obj }); // also starts the trigger's animation
                }
            }
        }
    }
}

/// VPX's `TriggerLineSeg`: one side of a polygon trigger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerLineSeg {
    pub seg: LineSeg,
    /// The trigger part's `m_d.m_enabled` (VPX reads it at each hit test).
    pub trigger_enabled: bool,
}

impl TriggerLineSeg {
    pub fn new(p1: Vertex2D, p2: Vertex2D, trigger_enabled: bool) -> Self {
        Self { seg: LineSeg::new(p1, p2), trigger_enabled }
    }

    /// VPX's `TriggerLineSeg::HitTest`.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !self.trigger_enabled {
            return -1.0;
        }

        // approach either face, not lateral-rolling point (assume center), not a rigid body contact
        self.seg.hit_test_basic(base, ball, dtime, coll, false, false, false)
    }

    /// VPX's `TriggerLineSeg::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
        if base.obj_type != ObjType::Trigger {
            return;
        }
        trigger_collide(base, coll, pball, env);
    }
}

/// VPX's `TriggerHitCircle`: a round trigger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerHitCircle {
    pub circle: HitCircle,
}

impl TriggerHitCircle {
    pub fn new(center: Vertex2D, radius: f32) -> Self {
        Self { circle: HitCircle::new(center, radius) }
    }

    /// VPX's `TriggerHitCircle::HitTest`: any face, not lateral, not rigid.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        self.circle.hit_test_basic_radius(base, ball, dtime, coll, false, false, false)
    }

    /// VPX's `TriggerHitCircle::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
        if base.obj_type < ObjType::Trigger {
            return; // triggers and kickers
        }
        trigger_collide(base, coll, pball, env);
    }
}
