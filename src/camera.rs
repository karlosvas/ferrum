use cgmath::{Matrix4, Vector3};
use winit::keyboard::KeyCode;

use crate::structs::{Camera, CameraController, CameraUniform};

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0)
);

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view: Matrix4<f32> = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj: Matrix4<f32> = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera){
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self { 
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            if_left_pressed: false,
            if_right_pressed: false,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_forward_pressed = is_pressed;
                true
            },
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.if_left_pressed = is_pressed;
                true
            },
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            },
            KeyCode::KeyD | KeyCode::ArrowRight =>{
                self.if_right_pressed = is_pressed;
                true
            },
            _ => false
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        use cgmath::InnerSpace;
        let forward: Vector3<f32> = camera.target - camera.eye;
        let forward_norm: Vector3<f32> = forward.normalize();
        let forward_mag: f32 = forward.magnitude();

        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed{
            camera.eye -= forward_norm * self.speed;
        }

        let right: Vector3<f32> = forward_norm.cross(camera.up);
        let forward: Vector3<f32> = camera.target - camera.eye;
        let forward_mag: f32 = forward.magnitude();

        if self.if_right_pressed{
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }

        if self.if_left_pressed{
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
}