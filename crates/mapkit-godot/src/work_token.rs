use godot::prelude::*;
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub(crate) struct MapKitWorkToken {
    base: Base<RefCounted>,
    token: mapkit_core::cancellation::CancellationToken,
}
#[godot_api]
impl IRefCounted for MapKitWorkToken {
    fn init(base: Base<RefCounted>) -> Self { Self { base, token: Default::default() } }
}
#[godot_api]
impl MapKitWorkToken {
    #[func] fn cancel(&self) { self.token.cancel(); }
    #[func] fn is_cancelled(&self) -> bool { self.token.is_cancelled() }
    #[func] fn enter(&self) { self.token.enter(); }
    #[func] fn leave(&self) { mapkit_core::cancellation::CancellationToken::leave(); }
}
