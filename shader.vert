#version 450

layout(location = 0) in vec2 inPos;
out gl_PerVertex
{
  vec4 gl_Position;
  float gl_PointSize;
};

layout(push_constant) uniform mdl {
  mat4 model;
};

void main() {
  gl_Position = model * vec4(inPos, 0.0, 1.0);
  gl_PointSize = 5.0;
}
