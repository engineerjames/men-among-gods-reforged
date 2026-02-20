use mag_core::traits;

/// Maps a character class/sex pair to the sprite ID used in the character selection list.
///
/// This is a UI-only mapping (it does not affect server-side appearance). For any unsupported
/// combination, it falls back to the mercenary male sprite.
pub fn get_sprite_id_for_class_and_sex(class: traits::Class, sex: traits::Sex) -> usize {
    match (class, sex) {
        (traits::Class::Harakim, traits::Sex::Male) => 4048,
        (traits::Class::Templar, traits::Sex::Male) => 2000,
        (traits::Class::Mercenary, traits::Sex::Male) => 5072,
        (traits::Class::Harakim, traits::Sex::Female) => 6096,
        (traits::Class::Templar, traits::Sex::Female) => 8144,
        (traits::Class::Mercenary, traits::Sex::Female) => 7120,
        _ => 5072,
    }
}
