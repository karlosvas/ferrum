use cgmath::{Matrix4, Quaternion, Vector3};

pub struct TransformDelta {
    pub translation: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
}

impl TransformDelta {
    pub fn to_matrix(&self) -> Matrix4<f32> {
        Matrix4::from_translation(self.translation)
            * Matrix4::from(self.rotation)
            * Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
    }
}
