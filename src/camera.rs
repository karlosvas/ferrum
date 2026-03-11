use crate::structs::{Camera, CameraController, CameraUniform};
use {
    cgmath::{Matrix4, Vector3},
    wgpu::{BindGroup, BindGroupLayout, Buffer, Device},
    winit::keyboard::KeyCode,
};

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
        let proj: Matrix4<f32> =
            cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }

    pub fn build_camera_setup(
        camera: &Camera,
        device: &Device,
        layout: &BindGroupLayout,
    ) -> (BindGroup, Buffer, CameraController, CameraUniform) {
        let mut uniform: CameraUniform = CameraUniform::new();
        uniform.update_view_proj(&camera);

        use wgpu::util::DeviceExt;
        let buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        // Controlador de  la cámara
        let camera_controller: CameraController = CameraController::new(4.0);

        (bind_group, buffer, camera_controller, uniform)
    }
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
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
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.if_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.if_right_pressed = is_pressed;
                true
            }
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera, dt: web_time::Duration) {
        use cgmath::InnerSpace;

        let dt: f32 = dt.as_secs_f32();

        let forward: Vector3<f32> = camera.target - camera.eye;
        let forward_norm: Vector3<f32> = forward.normalize();
        let forward_mag: f32 = forward.magnitude();

        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed * dt;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed * dt;
        }

        let right: Vector3<f32> = forward_norm.cross(camera.up);
        let forward: Vector3<f32> = camera.target - camera.eye;
        let forward_mag: f32 = forward.magnitude();

        if self.if_right_pressed {
            camera.eye =
                camera.target - (forward + right * self.speed * dt).normalize() * forward_mag;
        }

        if self.if_left_pressed {
            camera.eye =
                camera.target - (forward - right * self.speed * dt).normalize() * forward_mag;
        }
    }
}
