use concerto_core::ModelManager;
use serde_json::json;

fn main() -> Result<(), concerto_core::ConcertoError> {
    let model_json = r#"
    {
      "$class": "concerto.metamodel@1.0.0.Model",
      "namespace": "org.company@1.0.0",
      "declarations": [
        {
          "$class": "concerto.metamodel@1.0.0.EnumDeclaration",
          "name": "Department",
          "properties": [
            {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "ENGINEERING"},
            {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "HR"},
            {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "SALES"}
          ]
        },
        {
          "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
          "name": "Person",
          "isAbstract": false,
          "properties": [
            {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "email", "isArray": false, "isOptional": false},
            {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "name", "isArray": false, "isOptional": false}
          ]
        },
        {
          "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
          "name": "Employee",
          "isAbstract": false,
          "superType": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Person"},
          "properties": [
            {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "employeeId", "isArray": false, "isOptional": false},
            {
              "$class": "concerto.metamodel@1.0.0.IntegerProperty",
              "name": "age",
              "isArray": false,
              "isOptional": false,
              "validator": {
                "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
                "lower": 18,
                "upper": 65
              }
            },
            {
              "$class": "concerto.metamodel@1.0.0.ObjectProperty",
              "name": "department",
              "isArray": false,
              "isOptional": false,
              "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Department"}
            }
          ]
        }
      ]
    }
    "#;

    let valid_employee = json!({
        "$class": "org.company@1.0.0.Employee",
        "email": "bob@company.com",
        "name": "Bob",
        "employeeId": "E-101",
        "age": 28,
        "department": "ENGINEERING"
    });

    let invalid_employee = json!({
        "$class": "org.company@1.0.0.Employee",
        "email": "bob@company.com",
        "employeeId": "E-101",
        "age": 70,
        "department": "MARKETING",
        "nickname": "bobby"
    });

    let mut mm = ModelManager::new();
    mm.add_model_from_json(model_json)?;

    // nice polymorphic case: actual instance is Employee, requested type is Person
    let valid_result = mm.validate_instance(&valid_employee, "org.company@1.0.0.Person")?;
    println!("valid employee as Person:");
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "valid": valid_result.valid,
            "errors": valid_result.errors
        }))
        .expect("pretty print should work")
    );
    println!();

    let invalid_result = mm.validate_instance(&invalid_employee, "org.company@1.0.0.Employee")?;
    println!("invalid employee as Employee:");
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "valid": invalid_result.valid,
            "errors": invalid_result.errors
        }))
        .expect("pretty print should work")
    );

    Ok(())
}
