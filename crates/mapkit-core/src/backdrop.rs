//! Display-only scenery presets for standalone track maps. Consumers render them
//! around the venue; geometry, collision and the world content hash ignore them.

/// The RC arena first (the default when a map stores none), then the seven
/// default-world themes.
pub const IDS: [&str; 8] = ["rc_arena", "village", "neon_harbor", "deep_forest", "red_canyon", "snow_mountain", "machine_factory", "sky_park"];
