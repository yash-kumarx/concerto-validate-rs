/**
 * complex_node.mjs  —  concerto-wasm Node.js complex example
 *
 * Run after:
 *   wasm-pack build --target nodejs --out-dir pkg-node   (from concerto-wasm/)
 *   node scripts/complex_node.mjs
 *
 * Exercises:
 *   - abstract base (Person) + two levels of inheritance (Employee → Manager)
 *   - enum properties (Gender, EmploymentType)
 *   - string scalar with regex (Email, EmployeeId)
 *   - numeric bounds on salary (Double) and teamSize (Integer)
 *   - DateTime properties
 *   - multiple simultaneous validation errors
 *   - WasmModelManager.addModelValue  +  validateInstanceValue
 */

import { WasmModelManager } from "../pkg-node/concerto_wasm.js";

// ── model ──────────────────────────────────────────────────────────────────
const HR_MODEL = {
  $class: "concerto.metamodel@1.0.0.Model",
  namespace: "org.hr@1.0.0",
  declarations: [
    {
      $class: "concerto.metamodel@1.0.0.EnumDeclaration",
      name: "Gender",
      properties: [
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "MALE" },
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "FEMALE" },
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "OTHER" },
      ],
    },
    {
      $class: "concerto.metamodel@1.0.0.EnumDeclaration",
      name: "EmploymentType",
      properties: [
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "FULL_TIME" },
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "PART_TIME" },
        { $class: "concerto.metamodel@1.0.0.EnumProperty", name: "CONTRACT" },
      ],
    },
    {
      $class: "concerto.metamodel@1.0.0.StringScalar",
      name: "Email",
      validator: {
        $class: "concerto.metamodel@1.0.0.StringRegexValidator",
        pattern: "^[\\w.+-]+@[\\w-]+\\.[\\w.]+$",
        flags: "",
      },
    },
    {
      $class: "concerto.metamodel@1.0.0.StringScalar",
      name: "EmployeeId",
      validator: {
        $class: "concerto.metamodel@1.0.0.StringRegexValidator",
        pattern: "^EMP-[0-9]{4}$",
        flags: "",
      },
    },
    {
      $class: "concerto.metamodel@1.0.0.ConceptDeclaration",
      name: "Person",
      isAbstract: true,
      properties: [
        {
          $class: "concerto.metamodel@1.0.0.ObjectProperty",
          name: "email",
          isArray: false,
          isOptional: false,
          type: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "Email" },
        },
        {
          $class: "concerto.metamodel@1.0.0.StringProperty",
          name: "firstName",
          isArray: false,
          isOptional: false,
          lengthValidator: {
            $class: "concerto.metamodel@1.0.0.StringLengthValidator",
            minLength: 1,
            maxLength: 50,
          },
        },
        {
          $class: "concerto.metamodel@1.0.0.StringProperty",
          name: "lastName",
          isArray: false,
          isOptional: false,
        },
        {
          $class: "concerto.metamodel@1.0.0.DateTimeProperty",
          name: "dateOfBirth",
          isArray: false,
          isOptional: false,
        },
        {
          $class: "concerto.metamodel@1.0.0.ObjectProperty",
          name: "gender",
          isArray: false,
          isOptional: true,
          type: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "Gender" },
        },
      ],
    },
    {
      $class: "concerto.metamodel@1.0.0.ConceptDeclaration",
      name: "Employee",
      isAbstract: false,
      superType: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "Person" },
      properties: [
        {
          $class: "concerto.metamodel@1.0.0.ObjectProperty",
          name: "employeeId",
          isArray: false,
          isOptional: false,
          type: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "EmployeeId" },
        },
        {
          $class: "concerto.metamodel@1.0.0.DoubleProperty",
          name: "salary",
          isArray: false,
          isOptional: false,
          validator: {
            $class: "concerto.metamodel@1.0.0.DoubleDomainValidator",
            lower: 20000.0,
            upper: 500000.0,
          },
        },
        {
          $class: "concerto.metamodel@1.0.0.BooleanProperty",
          name: "isActive",
          isArray: false,
          isOptional: false,
        },
        {
          $class: "concerto.metamodel@1.0.0.ObjectProperty",
          name: "employmentType",
          isArray: false,
          isOptional: false,
          type: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "EmploymentType" },
        },
        {
          $class: "concerto.metamodel@1.0.0.DateTimeProperty",
          name: "hiredAt",
          isArray: false,
          isOptional: false,
        },
      ],
    },
    {
      $class: "concerto.metamodel@1.0.0.ConceptDeclaration",
      name: "Manager",
      isAbstract: false,
      superType: { $class: "concerto.metamodel@1.0.0.TypeIdentifier", name: "Employee" },
      properties: [
        {
          $class: "concerto.metamodel@1.0.0.StringProperty",
          name: "department",
          isArray: false,
          isOptional: false,
        },
        {
          $class: "concerto.metamodel@1.0.0.IntegerProperty",
          name: "teamSize",
          isArray: false,
          isOptional: false,
          validator: {
            $class: "concerto.metamodel@1.0.0.IntegerDomainValidator",
            lower: 1,
            upper: 200,
          },
        },
      ],
    },
  ],
};

// ── instances ──────────────────────────────────────────────────────────────
const VALID_MANAGER = {
  $class: "org.hr@1.0.0.Manager",
  email: "priya.sharma@acme.com",
  firstName: "Priya",
  lastName: "Sharma",
  dateOfBirth: "1988-04-15T00:00:00Z",
  gender: "FEMALE",
  employeeId: "EMP-0042",
  salary: 145000.0,
  isActive: true,
  employmentType: "FULL_TIME",
  hiredAt: "2015-09-01T09:00:00Z",
  department: "Engineering",
  teamSize: 12,
};

// 8 deliberate errors:
//   email     → fails regex
//   firstName → fails minLength (empty string)
//   dateOfBirth → not a datetime
//   gender    → not a valid enum value
//   employeeId → fails regex (no EMP- prefix)
//   salary    → below lower bound (5000 < 20000)
//   isActive  → wrong type (string instead of Boolean)
//   employmentType → not a valid enum value
//   teamSize  → above upper bound (999 > 200)
//   unknownField → extra property not in schema
const INVALID_MANAGER = {
  $class: "org.hr@1.0.0.Manager",
  email: "not-an-email",
  firstName: "",
  lastName: "Sharma",
  dateOfBirth: "not-a-date",
  gender: "NONBINARY",
  employeeId: "BADID",
  salary: 5000.0,
  isActive: "yes",
  employmentType: "GIGS",
  hiredAt: "2015-09-01T09:00:00Z",
  department: "Engineering",
  teamSize: 999,
  unknownField: "oops",
};

// ── helpers ────────────────────────────────────────────────────────────────
function printResult(label, result) {
  console.log(`\n── ${label} ──`);
  if (result.valid) {
    console.log("  ✓ VALID");
  } else {
    console.log(`  ✗ INVALID  (${result.errors.length} errors)`);
    for (const e of result.errors) {
      console.log(`    [${e.path}]  ${e.message}`);
    }
  }
}

function assert(cond, msg) {
  if (!cond) throw new Error(`assertion failed: ${msg}`);
}

// ── main ───────────────────────────────────────────────────────────────────
const mm = new WasmModelManager();
mm.addModelValue(HR_MODEL);

const validResult = mm.validateInstanceValue(VALID_MANAGER, "org.hr@1.0.0.Manager");
printResult("valid Manager (all fields correct)", validResult);
assert(validResult.valid, "expected valid Manager to pass");

const invalidResult = mm.validateInstanceValue(INVALID_MANAGER, "org.hr@1.0.0.Manager");
printResult("invalid Manager (8+ deliberate errors)", invalidResult);
assert(!invalidResult.valid, "expected invalid Manager to fail");
assert(invalidResult.errors.length >= 5, "expected at least 5 errors");

// also validate an Employee (one level up in chain) to confirm inheritance lookup
const VALID_EMPLOYEE = {
  $class: "org.hr@1.0.0.Employee",
  email: "bob.dev@acme.com",
  firstName: "Bob",
  lastName: "Dev",
  dateOfBirth: "1995-11-20T00:00:00Z",
  employeeId: "EMP-0099",
  salary: 88000.0,
  isActive: true,
  employmentType: "CONTRACT",
  hiredAt: "2022-01-10T08:30:00Z",
};
const empResult = mm.validateInstanceValue(VALID_EMPLOYEE, "org.hr@1.0.0.Employee");
printResult("valid Employee (no gender, no manager fields)", empResult);
assert(empResult.valid, "expected valid Employee to pass");

console.log("\nAll assertions passed.");
