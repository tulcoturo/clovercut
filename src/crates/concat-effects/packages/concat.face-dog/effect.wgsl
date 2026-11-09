struct Params { amount:f32, size:f32 }
fn decorate_face(original:vec4<f32>,source:vec2<f32>,face:TrackedFace)->vec4<f32> {
 var base=original;
 if(face.mouth_score.z<0.7){return base;}
  let size=params.size*0.01;
  let p=face_local(source,face)/size;
  let sway=sin(frame.clip_time*3.0)*0.035;
  let ear_left=face_ellipse(p-vec2<f32>(-1.08+sway,-0.38),vec2<f32>(0.34,0.8));
  let ear_right=face_ellipse(p-vec2<f32>(1.08-sway,-0.38),vec2<f32>(0.34,0.8));
  let ears=face_cover(min(ear_left,ear_right));
  let inner=face_cover(min(face_ellipse(p-vec2<f32>(-1.08+sway,-0.31),vec2<f32>(0.19,0.55)),face_ellipse(p-vec2<f32>(1.08-sway,-0.31),vec2<f32>(0.19,0.55))));
  base=face_over(base,mix(vec3<f32>(0.21,0.085,0.028),vec3<f32>(0.47,0.2,0.13),inner),ears*params.amount*0.01);
  let nose=face_local(face.nose_mouth.xy,face)/size;
  let q=p-nose;
  let muzzle=face_cover(min(face_ellipse(q-vec2<f32>(-0.22,0.18),vec2<f32>(0.35,0.27)),face_ellipse(q-vec2<f32>(0.22,0.18),vec2<f32>(0.35,0.27))));
  base=face_over(base,vec3<f32>(0.78,0.56,0.32),muzzle*params.amount*0.01);
  let nose_shape=face_cover(face_ellipse(q,vec2<f32>(0.22,0.14)));
  let highlight=face_cover(face_ellipse(q-vec2<f32>(-0.06,-0.04),vec2<f32>(0.055,0.027)));
  base=face_over(base,vec3<f32>(0.012)+vec3<f32>(0.15)*highlight,nose_shape*params.amount*0.01);
  let line=face_cover(face_round_box(q-vec2<f32>(0.0,0.19),vec2<f32>(0.022,0.09),0.015));
  base=face_over(base,vec3<f32>(0.025),line*params.amount*0.01);
 return base;
}
fn effect(uv:vec2<f32>)->vec4<f32> {
 var base=sample(uv);
 if(params.amount<=0.0){return base;}
 let source=face_source(uv);
 // Fixed calls avoid dynamic indexing/loop lowering in Windows WARP/FXC.
 base=decorate_face(base,source,face_inputs.faces[0]);
 base=decorate_face(base,source,face_inputs.faces[1]);
 base=decorate_face(base,source,face_inputs.faces[2]);
 base=decorate_face(base,source,face_inputs.faces[3]);
 return base;
}
