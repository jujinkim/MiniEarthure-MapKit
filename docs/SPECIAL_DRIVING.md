# Parametric special driving structures

Owner of effect kinds, parametric tracks, air-ring defaults and pipe dimensions.
General gimmick records follow [CURRENT_V1](../spec/CURRENT_V1.md#declarative-driving-structures).

## Effects

`target_speed`, `jump_height` and `air_ring` motion kinds carry an `effect`:

| Field | Range | Notes |
| --- | --- | --- |
| `strength_percent` | 1–100 | default 100 |
| `jump_height_cm` | 10–1000 | default 300; still-air horizontal launch height |
| `ring_radius_cm` | 75–600 | inner radius; see [air rings](#air-ring-defaults) |

Forward is local +Z in map coordinates; launch normal is local +Y. Physics
consumers account for gravity and body drag. `boost`/`launch` impulses keep their
meaning. Activation, aperture crossing, cooldown and vehicle speed belong to the
consumer.

## Parametric tracks

A static definition may contain `track` instead of parts (empty `parts`, identity
scale):

| Kind | Radius | Other |
| --- | --- | --- |
| `loop` | 150–600 cm | stored `width_cm` 140–600; new ribbons 400–600 cm |
| `cylinder` | see [pipe dimensions](#pipe-dimensions) | `length_cm` 600–3200 controls the axis |
| `swept_cylinder` | see [pipe dimensions](#pipe-dimensions) | 2–512 `centerline` floor/normal/forward frames |

- Unused dimensions are retained for switching templates. Editor template
  defaults: loop radius 250 cm, cylinder radius 200 cm, width 400 cm, length 1600 cm.
- Loop ends are separated; floor radius is 1.6× and crown radius 0.4× the radius
  parameter. Cylinders have flared entrance/exit, a hollow interior and no caps.
- `special_track.rs` produces the only geometry for presentation, collision and
  per-tile occupied bounds. Inner faces and outer shell/sides are distinct; only
  inner faces may provide curved wheel support. Neither is a spawn/recovery
  surface, and spawn APIs exclude ordinary surfaces inside the track. Material
  and curved-driving role are independent.
- Faces use integer tenths of a millimetre (`units_per_metre: 10000`); authored
  coordinates and occupied bounds stay in cm. Fixed libm sampling, 256 angular
  sections and 16/32/64 adaptive cylinder bands keep adjacent normals within
  5° and approximation error below 1 cm.
- Source/archive records store parameters, not faces. Godot `resolve_gimmick` and
  packed output expand `track_mesh` plus `memory_bytes`; this is a bridge view, not
  a file format.
- Cost: 16 KiB per definition + 32 KiB per convex part + 4 KiB per track tile.
  Spatial selection, regional dependencies, occupied bounds and the driving window
  include the safety rectangle. No 32-part, memory or cell limit is raised.

## Pipe dimensions

- New cylinder and swept-cylinder bores are 4/6 m (radii 200/300 cm), default 4 m.
  New road and pipe ports have a 400 cm minimum. Generation draws the two bores
  uniformly at every difficulty. `banked_chicane` halfpipes are 4 or 6 m wide;
  their swept halfpipe uses radius width/2 (200/300 cm) and width `width_cm`.
  Runtime physics of the 300 cm halfpipe is unverified; if it fails, generation
  keeps `banked_chicane` at 4 m and varies only its shape.
- Curved pipes use centreline radius `max(400, bore/2 + 200)` cm. General corners
  and spiral radius/rise formulas keep their existing definitions.
- The catalogue publishes `min_width_cm`, `loop_min_width_cm`,
  `loop_default_width_cm`, `pipe_min_radius_cm`, `pipe_default_radius_cm` and
  `pipe_radii_cm`. New standalone loops/pipes and geometry edits call
  `SpecialTrack::validate_authoring` / Godot `validate_special_track_authoring`.
- Stored v1 parameters keep their original accepted ranges (pipe radius 100–600 cm).
  Loading, preview, bounds and Undo/Redo use stored-data validation. New authoring
  errors name the location and dimension; no implicit resize or conversion occurs.

## Pipe control points

`cylinder` and `banked_chicane` instances may carry local cubic `control_points`
(v1 source field shared with free/flight curves). The compiler then sweeps
`geometry::pipe_bezier` instead of the preset centreline: frames at most 50 cm
apart, gravity-aligned, `tube_radius = lateral = width/2`, and the gimmick
centreline is that materialized local path. Rules (`pipe_control_issue`,
`E_PIPE_DIMENSIONS`, source unmodified):

- width 400 or 600 cm; 4+3k points with k ≤ 6;
- `cp[0]` is the origin and `cp[1]` lies on +Z; every coordinate within ±6400 cm;
- minimum 3D curvature radius max(400, width/2+200) cm (1% allowance for cubic
  arcs and eased rise); |grade| ≤ 15%; level end tangent; ≤512 frames.

The catalogue publishes these as `pipe_control_points`. Without control points the
preset shapes are unchanged.

## Pipe material

New assembled cylinders and the standalone straight-cylinder template use
`#596168`. `gimmick_geometry.pipe_material` sets roughness 0.82, metallic 0.65 and
no emission in production display and Editor previews. Stored colours are
authoritative; loading never recolours a saved object. Geometry, collision and
contact properties are unaffected. Selection highlights a private preview material
and restores the stored colour on deselect.

## Air ring defaults

- New automatic track rings, manual air-ring actions and the standalone template use
  a 150 cm inner radius (3 m opening) and 100% target speed.
- Square track rim: 50 cm width / 30 cm depth. Standalone 24-sector round rim:
  20 cm width / depth. Cooldown is 1500 ms.
- The declared vertices drive display, preview and collision. Explicit saved
  dimensions and strengths are preserved.

## Validation

Focused `special_tracks`, `pipe_dimensions` and gimmick Rust tests plus the Godot
`pipe_material_validator`. Physical driving and effects are consumer/user checks.
