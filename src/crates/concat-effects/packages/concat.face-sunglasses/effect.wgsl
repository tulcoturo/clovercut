struct Params { amount:f32, size:f32 }
fn decorate_face(original:vec4<f32>,source:vec2<f32>,face:TrackedFace)->vec4<f32> {
 var base=original;
 if(face.mouth_score.z<0.7){return base;}
  let p=face_local(source,face)/(params.size*0.01);
  let left=face_round_box(p-vec2<f32>(-0.5,0.03),vec2<f32>(0.43,0.29),0.12);
  let right=face_round_box(p-vec2<f32>(0.5,0.03),vec2<f32>(0.43,0.29),0.12);
  let lens=min(left,right);
  let bridge=face_round_box(p-vec2<f32>(0.0,-0.035),vec2<f32>(0.15,0.055),0.025);
  let arms=face_round_box(p-vec2<f32>(0.0,-0.15),vec2<f32>(1.08,0.045),0.03);
  let frame=face_cover(min(lens,min(bridge,arms)));
  let glass=face_cover(lens+0.045);
  let shine=(1.0-smoothstep(0.015,0.055,abs(p.x+p.y*0.7-0.42)))*glass;
  let colour=mix(vec3<f32>(0.012),vec3<f32>(0.015,0.055,0.09),glass)+vec3<f32>(0.16,0.28,0.34)*shine;
  base=face_over(base,colour,frame*params.amount*0.01);
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
