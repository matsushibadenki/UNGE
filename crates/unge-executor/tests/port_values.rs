use unge_core::{Cardinality, DataType, Port};
use unge_executor::{Value, math_registry};
#[test]
fn maps_validate_unknown_required_cardinality_and_nonfinite_values() {
    let registry = math_registry();
    let mut d = registry.definition("math.number").unwrap().clone();
    d.inputs = vec![Port {
        name: "v".into(),
        data_type: DataType::Float,
        cardinality: Cardinality::Single,
        required: true,
    }];
    assert!(d.validate_inputs(&Default::default()).is_err());
    assert!(d.validate_inputs(&[("v".into(), vec![])].into()).is_err());
    assert!(
        d.validate_inputs(&[("v".into(), vec![Value::Float(1.)])].into())
            .is_ok()
    );
    assert!(
        d.validate_inputs(&[("v".into(), vec![Value::Float(1.), Value::Float(2.)])].into())
            .is_err()
    );
    assert!(
        d.validate_inputs(&[("v".into(), vec![Value::Float(f64::INFINITY)])].into())
            .is_err()
    );
    assert!(
        d.validate_outputs(&[("value".into(), Value::Float(f64::NAN))].into())
            .is_err()
    );
    d.inputs.push(d.inputs[0].clone());
    assert!(d.validate_inputs(&Default::default()).is_err());
    #[cfg(feature = "schema")]
    assert!(d.input_values_schema().is_err());
}
