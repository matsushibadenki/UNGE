//! Host-owned example showing reusable property widgets without adding domain logic to core.
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};
use unge_executor::*;

pub fn registry() -> Registry {
    let mut registry = math_registry();
    let mut definition = registry.definition("math.number").unwrap().clone();
    definition.type_id = "example.adjust".into();
    definition.version = "1".into();
    let text = |en: &str, ja: &str, zh: &str| LocalizedText {
        en: en.into(),
        ja: ja.into(),
        zh_cn: zh.into(),
    };
    definition.name = text("Adjust value", "数値を調整", "调整数值");
    definition.description = text(
        "Scale, offset and round",
        "倍率・加算値・丸めを設定",
        "设置倍率、偏移和取整",
    );
    definition.inputs = definition.outputs.clone();
    let field = |name, description, value_type, default, required| PropertyDefinition {
        name,
        description,
        value_type,
        default: Some(default),
        required,
    };
    definition.property_schema = PropertySchema {
        additional_properties: false,
        fields: BTreeMap::from([
            (
                "gain".into(),
                field(
                    text("Gain", "倍率", "倍率"),
                    text("Multiply the input", "入力値に掛ける倍率", "输入的乘数"),
                    PropertyType::Float {
                        minimum: Some(0.0),
                        maximum: Some(4.0),
                    },
                    1.0.into(),
                    true,
                ),
            ),
            (
                "offset".into(),
                field(
                    text("Offset", "加算値", "偏移"),
                    text(
                        "Add after scaling",
                        "倍率を掛けた後に加算",
                        "相乘后加上此值",
                    ),
                    PropertyType::Int {
                        minimum: Some(-100),
                        maximum: Some(100),
                    },
                    0.into(),
                    true,
                ),
            ),
            (
                "rounding".into(),
                field(
                    text("Rounding", "丸め処理", "取整"),
                    text(
                        "none / round / floor",
                        "none: なし / round: 四捨五入 / floor: 切り下げ",
                        "none: 不取整 / round: 四舍五入 / floor: 向下取整",
                    ),
                    PropertyType::String {
                        min_length: 1,
                        max_length: Some(5),
                        choices: Some(vec!["none".into(), "round".into(), "floor".into()]),
                    },
                    "none".into(),
                    true,
                ),
            ),
            (
                "enabled".into(),
                field(
                    text("Enabled", "処理を有効にする", "启用处理"),
                    text(
                        "When off, pass the input through",
                        "オフのときは入力値をそのまま出力",
                        "关闭时直接输出输入值",
                    ),
                    PropertyType::Bool,
                    true.into(),
                    true,
                ),
            ),
            (
                "note".into(),
                field(
                    text("Note", "メモ", "备注"),
                    text(
                        "An optional note saved with this node",
                        "ノードと一緒に保存する任意のメモ",
                        "随节点保存的可选备注",
                    ),
                    PropertyType::String {
                        min_length: 0,
                        max_length: Some(240),
                        choices: None,
                    },
                    "".into(),
                    false,
                ),
            ),
        ]),
    };
    registry.register(definition, Arc::new(Adjust)).unwrap();
    registry
}
struct Adjust;
impl NodeExecutor for Adjust {
    fn execute(
        &self,
        ctx: ExecutionContext,
        inputs: Inputs,
    ) -> Pin<Box<dyn Future<Output = Result<Outputs, String>> + Send + '_>> {
        Box::pin(async move {
            let Some(Value::Float(input)) = inputs.get("value").and_then(|v| v.first()) else {
                return Err("missing input value".into());
            };
            let mut value = *input;
            if ctx.properties["enabled"]
                .as_bool()
                .ok_or("invalid enabled")?
            {
                value = value * ctx.properties["gain"].as_f64().ok_or("invalid gain")?
                    + ctx.properties["offset"].as_f64().ok_or("invalid offset")?;
                value = match ctx.properties["rounding"].as_str() {
                    Some("round") => value.round(),
                    Some("floor") => value.floor(),
                    Some("none") => value,
                    _ => return Err("invalid rounding".into()),
                };
            }
            if !value.is_finite() {
                return Err("adjustment overflow".into());
            }
            Ok(BTreeMap::from([("value".into(), Value::Float(value))]))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjustment_executes_schema_values_and_bypass() {
        let registry = registry();
        let definition = registry.definition("example.adjust").unwrap();
        let mut properties = definition.instantiate().properties;
        properties.insert("gain".into(), 1.5.into());
        properties.insert("offset".into(), 2.into());
        properties.insert("rounding".into(), "floor".into());
        definition.property_schema.validate(&properties).unwrap();
        for (enabled, expected) in [(true, 9.0), (false, 5.0)] {
            properties.insert("enabled".into(), enabled.into());
            let output = pollster::block_on(Adjust.execute(
                ExecutionContext {
                    node: unge_core::Id::new_v4(),
                    properties: properties.clone(),
                    cancellation: Cancellation::default(),
                },
                BTreeMap::from([("value".into(), vec![Value::Float(5.0)])]),
            ))
            .unwrap();
            assert_eq!(output["value"], Value::Float(expected));
        }
        properties.insert("gain".into(), 5.0.into());
        assert!(definition.property_schema.validate(&properties).is_err());
    }
}
