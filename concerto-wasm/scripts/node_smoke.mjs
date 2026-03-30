import { WasmModelManager, validateInstanceWithModel } from "../pkg-node/concerto_wasm.js";

const model = {
  $class: "concerto.metamodel@1.0.0.Model",
  namespace: "org.example@1.0.0",
  declarations: [
    {
      $class: "concerto.metamodel@1.0.0.ConceptDeclaration",
      name: "Person",
      isAbstract: false,
      properties: [
        {
          $class: "concerto.metamodel@1.0.0.StringProperty",
          name: "email",
          isArray: false,
          isOptional: false,
        },
        {
          $class: "concerto.metamodel@1.0.0.IntegerProperty",
          name: "age",
          isArray: false,
          isOptional: false,
        },
      ],
    },
  ],
};

const validInstance = {
  $class: "org.example@1.0.0.Person",
  email: "alice@example.com",
  age: 30,
};

const invalidInstance = {
  $class: "org.example@1.0.0.Person",
  email: "alice@example.com",
  age: "thirty",
};

const mm = new WasmModelManager();
mm.addModelValue(model);

const validResult = mm.validateInstanceValue(validInstance, "org.example@1.0.0.Person");
if (!validResult.valid) {
  throw new Error(`expected valid instance to pass: ${JSON.stringify(validResult)}`);
}

const invalidResult = validateInstanceWithModel(
  model,
  invalidInstance,
  "org.example@1.0.0.Person",
);
if (invalidResult.valid || invalidResult.errors.length === 0) {
  throw new Error(`expected invalid instance to fail: ${JSON.stringify(invalidResult)}`);
}

console.log("node wasm smoke test passed");
