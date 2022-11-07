use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::{GameMessage, MesState, OutGameMessages, OutMessageState, MAX_OUT_DELTA_TIME, POS_EPSILON_QRT, ANGLE_EPSILON, OUT_ANGLE_EPSILON, MIN_OUT_DELTA_TIME};
use crate::network::PingList;
use crate::player::{ControlMove, PlayerData};
use crate::tank::{TankEntityes, WheelData};

use super::{utils::*, TankMoveTarget};

#[repr(C)]
#[derive(Serialize, Deserialize, Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Data {
    pub movement: Vec2,
    pub pos: Vec2,
    pub angle: f32,
}

impl Data {
    pub fn is_moved(&self) -> bool {
        return self.movement.x != 0. || self.movement.y != 0. 
    }
}

pub fn update_body_position_from_net(
    time: Res<Time>,
    ping: Res<PingList>,
    mut data_query: Query<(
        &GlobalTransform,
        //       ChangeTrackers<State<Message<Data>>>,
        &MesState<Data>,
  //      &mut ExternalImpulse,
        &mut ExternalForce,
        &mut Sleeping,
        &TankEntityes,
        &PlayerData,
    )>,
    mut wheel_data_query: Query<&mut WheelData>,
    mut target_query: Query<&mut Transform, With<TankMoveTarget>>,
) {
// TODO
    
    for (
        global_transform,
        state,
        /*tank_control_data, mut forces, mut impulse,*/
        mut force,
        mut sleeping,
        entityes,
        player,
    ) in data_query.iter_mut()
    {
        let (_scale, rotation, translation) = global_transform.to_scale_rotation_translation();
        let data = state.data;

        //try to compensate the ping delay
        let (target_angle, target_pos) = if data.is_moved() {
            let delta_time = (time.seconds_since_startup() - state.time) as f32 + ping.get_time(player.handle)*0.5;

            let angle = if data.movement.x != 0. {
                normalize(data.angle - data.movement.x*0.1*delta_time)
            } else {
                data.angle
            };

            let pos = Vec3::new(data.pos.x, translation.y, data.pos.y) + if data.movement.y != 0. {
                let delta_pos = Vec3::new(data.movement.x, 0., data.movement.y)*0.2*delta_time;

                Transform::from_rotation(Quat::from_axis_angle(Vec3::Y, data.angle))
                .compute_matrix()
    //            .inverse()
                .transform_point3(delta_pos)
            } else {
                Vec3::ZERO
            };

            (angle, pos)
        } else {
            (data.angle, Vec3::new(data.pos.x, translation.y, data.pos.y))
        }; 

        if let Ok(mut target_transform) = target_query.get_single_mut() {
            target_transform.translation = target_pos;
        }

  //      log::info!("src_pos:{} src_angle:{} trg_pos:{} trg_angle:{}",
  //                      translation, rotation.to_euler(EulerRot::YXZ).0, target_pos, target_angle);

        //correct body pos
        let delta_pos = Vec3::new(target_pos.x - translation.x, 0., target_pos.z - translation.z);

        //       log::info!("tank mod update_body_position translation.pos {} input.pos{} delta_pos{}",
        //           transform.translation, tank_control_body.pos, delta_pos);

        let length_squared = delta_pos.length_squared().min(3.);
        if length_squared > POS_EPSILON_QRT {                      
            force.force = delta_pos * ((1. + length_squared).powf(2.0) - 1.) * 10.;
        }

        let delta_angle = delta_dir(target_angle, rotation.to_euler(EulerRot::YXZ).0).min(0.1);
        if delta_angle.abs() > ANGLE_EPSILON {        
            let torque = ((1. + delta_angle).powf(2.0) - 1.) * 10.;
            force.torque = rotation.mul_vec3(Vec3::Y * torque);
        }

//        log::info!("update_body_position delta_pos: {}; tmp_impulse: {}; current_dir: {}; from_net.dir: {}; torque: {}", delta_pos, tmp_impulse, current_body_dir, data.angle, torque);

            let wheel_data_movement = if data.movement.length_squared() > 0.1 {
                sleeping.linear_threshold = 2.;
                sleeping.angular_threshold = 10.;
                sleeping.sleeping = false;
                Some(data.movement.clone())
            } else {
                sleeping.linear_threshold = 2.;
                sleeping.angular_threshold = 10.;
                sleeping.sleeping = true;
                //          sleeping.default();
                None
            };

            for wheel in &entityes.wheels {
                if let Ok(mut wheel_data) = wheel_data_query.get_component_mut::<WheelData>(*wheel)
                {
                    wheel_data.movement = wheel_data_movement.clone();

                    //           println!("player prep_wheel_input, ok");
                }
            }
 //       }
    }
}

//apply player control
pub fn update_player_body_control(
//    local_handles: Res<LocalHandles>,
    time: Res<Time>,
    mut query: Query<(
        &GlobalTransform,
        //       ChangeTrackers<PlayerControlMove>,
        &ControlMove,
//        &mut ExternalImpulse,
        &mut Sleeping,
        &TankEntityes,
    )>,
    //        &mut ExternalForce,
    mut out_data_state: ResMut<OutMessageState<Data>>,
    mut output: ResMut<OutGameMessages<GameMessage>>,
    mut wheel_data_query: Query<&mut WheelData>,
) {
    if query.is_empty() {
        return;
    }
    
    out_data_state.delta_time += time.delta_seconds();

    let (
        global_transform,
        //     tracker,
        control,
        /*tank_control_data, mut forces, mut impulse,*/
        mut sleeping,
        entityes,
    ) = query.single_mut();

    let (_scale, rotation, translation) = global_transform.to_scale_rotation_translation();
    let new_pos = Vec2::new(translation.x, translation.z);
    let new_dir = rotation.to_euler(EulerRot::YXZ).0;

    let is_moved = control.movement.x != 0. || control.movement.y != 0.;
    let is_changed =  normalize(new_dir - out_data_state.old_data.angle).abs() > OUT_ANGLE_EPSILON ||
                            (new_pos - out_data_state.old_data.pos).length_squared() > POS_EPSILON_QRT;
    let is_started_or_stoped = control.movement.x != out_data_state.old_data.movement.x ||
                                    control.movement.y != out_data_state.old_data.movement.y;

    if (is_changed && out_data_state.delta_time >= MIN_OUT_DELTA_TIME) || 
        (is_moved && out_data_state.delta_time >= MAX_OUT_DELTA_TIME) ||
        is_started_or_stoped {
        out_data_state.old_data.movement = control.movement;
        out_data_state.old_data.pos = new_pos;
        out_data_state.old_data.angle = new_dir;

        output.data.push(GameMessage::from(out_data_state.old_data));
        out_data_state.delta_time = 0.;
    }

    if is_started_or_stoped {
        let wheel_data_movement = if control.movement.length_squared() > 0.001 {
            sleeping.linear_threshold = -1.;
            sleeping.angular_threshold = -1.;
            sleeping.sleeping = false;
            Some(control.movement.clone())
        } else {
            sleeping.linear_threshold = 1.;
            sleeping.angular_threshold = 10.;
            sleeping.sleeping = true;
            //          sleeping.default();
            None
        };

        for wheel in &entityes.wheels {
            if let Ok(mut wheel_data) = wheel_data_query.get_component_mut::<WheelData>(*wheel) {
                wheel_data.movement = wheel_data_movement.clone();

                //           println!("player prep_wheel_input, ok");
            }
        }
    }
}
