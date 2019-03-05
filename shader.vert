#version 450

out gl_PerVertex
{
  vec4 gl_Position;
  float gl_PointSize;
};

layout(push_constant) uniform mdl {
  mat4 model;
};

void main() {
  gl_Position = model * vec4(0.0, 0.0, 0.0, 1.0);
  gl_PointSize = 5.0;
}
