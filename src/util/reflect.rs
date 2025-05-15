use bevy::prelude::*;

pub fn try_apply_parsed(
    field: &mut (dyn PartialReflect + 'static),
    val: &String,
) -> anyhow::Result<()> {
    match (*field).reflect_type_path() {
        "String" => field.try_apply(val)?,
        "f32" => field.try_apply(&val.parse::<f32>()?)?,
        "bool" => field.try_apply(&val.parse::<bool>()?)?,
        _ => todo!("Type not yet supported!"),
    }
    Ok(())
}
