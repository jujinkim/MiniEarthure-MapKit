//! Bounded parametric ribbons. All consumers use these quantized tenth-millimetre faces.
use crate::*;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind { Loop, Cylinder }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecialTrack {
    pub kind: TrackKind,
    pub radius_cm: u32,
    pub width_cm: u32,
    pub length_cm: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct TrackMesh {
    pub units_per_metre: u32,
    pub inner: Vec<[Vertex; 3]>,
    pub shell: Vec<[Vertex; 3]>,
    #[serde(skip)]
    pub tiles: Vec<(Vertex, Vertex)>,
}
const STEPS: usize = 256;
impl SpecialTrack {
    pub fn valid(&self) -> bool {
        (150..=600).contains(&self.radius_cm) && (140..=600).contains(&self.width_cm)
            && (600..=3200).contains(&self.length_cm)
    }
    fn bands(&self) -> usize { if self.kind == TrackKind::Loop {1} else if self.radius_cm * 2 > self.length_cm {64} else if self.radius_cm * 4 > self.length_cm {32} else {16} }
    pub fn tile_count(&self) -> usize { STEPS * self.bands() }
    pub fn bound_radius(&self) -> i64 {
        match self.kind {
            TrackKind::Loop => i64::from(self.radius_cm)*4 + i64::from(self.width_cm)*2 + 40,
            TrackKind::Cylinder => i64::from(self.radius_cm)*4 + i64::from(self.length_cm)/2 + 40,
        }
    }
    pub fn mesh(&self) -> TrackMesh {
        let mut mesh=TrackMesh{units_per_metre:10000,inner:vec![],shell:vec![],tiles:vec![]};
        let r=f64::from(self.radius_cm); let w=f64::from(self.width_cm); let l=f64::from(self.length_cm);
        let bands=self.bands();
        let point=|i:usize,j:usize,outer:bool| -> Vertex {
            let t=i as f64*std::f64::consts::TAU/STEPS as f64;
            let (sn,cs)=(libm::sin(t),libm::cos(t));
            let thickness=if outer {10.0} else {0.0};
            let p=match self.kind {
                TrackKind::Loop => {
                    // Integrate radius r*(1 + .6*cos(theta)): 1.6r at the floor,
                    // .4r at the crown. Open, separated ends avoid self-overlap.
                    // Smooth lateral separation keeps entrance and exit disjoint.
                    let smooth=|v:f64| {let u=v.clamp(0.0,1.0);u*u*(3.0-2.0*u)};
                    let shift=-1.0+smooth((t-0.85)/1.60)+smooth((t-3.83)/1.60);
                    let x=w*0.65*shift + (j as f64-0.5)*w;
                    [x,r*(1.0-cs)+0.30*r*sn*sn-thickness*cs,
                     r*(sn+0.60*(t*0.5+libm::sin(2.0*t)*0.25))+thickness*sn]
                },
                TrackKind::Cylinder => {
                    let u=j as f64/bands as f64;
                    // Flared entrances leave the bottom tangent to the access road.
                    let flare=0.12*r*(1.0+libm::cos(u*std::f64::consts::TAU))*0.5;
                    let radius=r+flare;
                    [(radius+thickness)*sn,radius-(radius+thickness)*cs,(u-0.5)*l]
                }
            };
            p.map(|v|libm::round(v*100.0) as i64)
        };
        for i in 0..STEPS { for j in 0..bands {
            let mut inside=[point(i,j,false),point(i+1,j,false),point(i+1,j+1,false),point(i,j+1,false)];
            let mut outside=[point(i,j,true),point(i+1,j,true),point(i+1,j+1,true),point(i,j+1,true)];
            // Cylinder and ribbon parametric axes have opposite handedness.
            if self.kind==TrackKind::Cylinder {inside.reverse();outside.reverse();}
            mesh.inner.extend([[inside[0],inside[1],inside[2]],[inside[0],inside[2],inside[3]]]);
            mesh.shell.extend([[outside[2],outside[1],outside[0]],[outside[3],outside[2],outside[0]]]);
            // Only exposed boundaries get sides; no internal contact seams.
            for edge in 0..4 {
                let exposed=if self.kind==TrackKind::Loop {edge==0 || edge==2 || (edge==3&&i==0) || (edge==1&&i+1==STEPS)}
                    else {(edge==0&&j+1==bands)||(edge==2&&j==0)};
                if exposed {let k=(edge+1)%4;mesh.shell.extend([[inside[edge],outside[edge],outside[k]],[inside[edge],outside[k],inside[k]]]);}
            }
            let all=inside.iter().chain(&outside);
            let low=std::array::from_fn(|a|all.clone().map(|v|v[a].div_euclid(100)).min().unwrap());
            let high=std::array::from_fn(|a|all.clone().map(|v|(v[a]+99).div_euclid(100)).max().unwrap());
            mesh.tiles.push((low,high));
        }}
        mesh
    }
}
