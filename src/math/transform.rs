use cgmath::{Matrix4, Quaternion, Vector3};

#[derive(Clone, Copy)]
pub struct TransformDelta {
    pub translation: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
}

impl TransformDelta {
    pub fn new(translation: Vector3<f32>, rotation: Quaternion<f32>, scale: Vector3<f32>) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn to_matrix(&self) -> Matrix4<f32> {
        Matrix4::from_translation(self.translation)
            * Matrix4::from(self.rotation)
            * Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
    }
}

impl Default for TransformDelta {
    fn default() -> Self {
        TransformDelta {
            translation: Vector3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}
