import {
  jsonUndefined,
  jsonNull,
  jsonBool,
  jsonNumber,
  jsonString,
  jsonArray,
  jsonObject,
  zodString,
  zodNumber,
  zodBoolean,
  zodLiteralString,
  zodLiteralNumber,
  zodLiteralBool,
  zodObject,
  zodArray,
  zodOptional,
  zodNullable,
  zodEnum,
  zodUnion,
  zodTuple,
  zodDefault,
  zodRecord,
  zodDiscriminatedUnion,
  stringMin,
  stringMax,
  stringEmail,
  numberMin,
  numberMax,
  safeParse as lilSafeParse,
} from "../../../build/zod-lilscript.js";

function fromJs(value) {
  if (value === undefined) return jsonUndefined();
  if (value === null) return jsonNull();
  if (typeof value === "boolean") return jsonBool(value);
  if (typeof value === "number") return jsonNumber(value);
  if (typeof value === "string") return jsonString(value);
  if (Array.isArray(value)) return jsonArray(value.map(fromJs));
  if (typeof value === "object") {
    const obj = new Map();
    const keys = Object.keys(value);
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i];
      obj.set(key, fromJs(value[key]));
    }
    return jsonObject(obj, keys);
  }
  return jsonUndefined();
}

function toJs(node) {
  if (node == null) return undefined;
  const kind = node.kind ?? node[0];
  if (kind === 0) return undefined;
  if (kind === 1) return null;
  if (kind === 2) return node.boolVal;
  if (kind === 3) return node.numVal;
  if (kind === 4) return node.strVal;
  if (kind === 5) return node.items.map(toJs);
  const obj = node.obj;
  const keys = node.keys;
  const out = {};
  for (let i = 0; i < keys.length; i += 1) {
    const key = keys[i];
    const value = obj instanceof Map ? obj.get(key) : obj[key];
    out[key] = toJs(value);
  }
  return out;
}

function issuePath(path) {
  if (!path) return [];
  return path.map((part) => part);
}

function wrap(schema) {
  return {
    __schema: schema,
    min(value) {
      if (schema.kind === 0) return wrap(stringMin(schema, value));
      return wrap(numberMin(schema, value));
    },
    max(value) {
      if (schema.kind === 0) return wrap(stringMax(schema, value));
      return wrap(numberMax(schema, value));
    },
    email() {
      return wrap(stringEmail(schema));
    },
    optional() {
      return wrap(zodOptional(schema));
    },
    nullable() {
      return wrap(zodNullable(schema));
    },
    default(value) {
      return wrap(zodDefault(schema, fromJs(value)));
    },
    transform(fn) {
      const base = wrap(schema);
      return {
        safeParse(data) {
          const result = base.safeParse(data);
          if (!result.success) return result;
          return { success: true, data: fn(result.data) };
        },
        parse(data) {
          const result = this.safeParse(data);
          if (result.success) return result.data;
          const error = new Error(JSON.stringify(result.error.issues));
          error.issues = result.error.issues;
          throw error;
        },
      };
    },
    safeParse(data) {
      const result = lilSafeParse(schema, fromJs(data));
      if (result.success) {
        return { success: true, data: toJs(result.data) };
      }
      const issues = result.issues.map((issue) => {
        const code = issue.code ?? issue[0];
        const message = issue.message ?? issue[1];
        const path = issuePath(issue.path ?? issue[2]);
        return { code, message, path };
      });
      return {
        success: false,
        error: { issues },
      };
    },
    parse(data) {
      const result = this.safeParse(data);
      if (result.success) return result.data;
      const error = new Error(JSON.stringify(result.error.issues));
      error.issues = result.error.issues;
      throw error;
    },
  };
}

const z = {
  string() {
    return wrap(zodString());
  },
  number() {
    return wrap(zodNumber());
  },
  boolean() {
    return wrap(zodBoolean());
  },
  literal(value) {
    if (typeof value === "string") return wrap(zodLiteralString(value));
    if (typeof value === "number") return wrap(zodLiteralNumber(value));
    return wrap(zodLiteralBool(value));
  },
  array(inner) {
    return wrap(zodArray(inner.__schema));
  },
  object(shape) {
    const map = new Map();
    const keys = Object.keys(shape);
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i];
      map.set(key, shape[key].__schema);
    }
    return wrap(zodObject(map, keys));
  },
  enum(values) {
    return wrap(zodEnum(values));
  },
  union(options) {
    return wrap(zodUnion(options.map((option) => option.__schema)));
  },
  tuple(items) {
    return wrap(zodTuple(items.map((item) => item.__schema)));
  },
  record(valueType) {
    return wrap(zodRecord(valueType.__schema));
  },
  discriminatedUnion(key, options) {
    const values = options.map((option) => {
      const field = option.__schema.shape.get(key);
      return field.literalStr;
    });
    return wrap(
      zodDiscriminatedUnion(
        key,
        options.map((option) => option.__schema),
        values,
      ),
    );
  },
};

function formatResult(result) {
  if (result.success) {
    if (result.data === undefined) return "ok:undefined";
    return `ok:${JSON.stringify(result.data)}`;
  }
  return `err:${result.error.issues
    .map((issue) => `${issue.code}@${issue.path.join("/")}:${issue.message}`)
    .join(";")}`;
}

function check(parts, passed, actual, expected) {
  parts.push(actual);
  if (actual === expected) passed.value += 1;
}

const passed = { value: 0 };
const parts = [];

check(parts, passed, formatResult(z.string().safeParse("hi")), 'ok:"hi"');
check(
  parts,
  passed,
  formatResult(z.string().safeParse(1)),
  "err:invalid_type@:Expected string, received number",
);
check(
  parts,
  passed,
  formatResult(z.string().min(3).safeParse("ab")),
  "err:too_small@:String must contain at least 3 character(s)",
);
check(
  parts,
  passed,
  formatResult(z.string().max(2).safeParse("abc")),
  "err:too_big@:String must contain at most 2 character(s)",
);
check(
  parts,
  passed,
  formatResult(z.string().email().safeParse("ab@c.com")),
  'ok:"ab@c.com"',
);
check(
  parts,
  passed,
  formatResult(z.string().email().safeParse("a@b.c")),
  "err:invalid_string@:Invalid email",
);
check(
  parts,
  passed,
  formatResult(z.string().email().safeParse("not-email")),
  "err:invalid_string@:Invalid email",
);
check(
  parts,
  passed,
  formatResult(z.string().email().min(5).safeParse("x")),
  "err:invalid_string@:Invalid email;too_small@:String must contain at least 5 character(s)",
);

check(parts, passed, formatResult(z.number().safeParse(1.5)), "ok:1.5");
check(
  parts,
  passed,
  formatResult(z.number().safeParse("x")),
  "err:invalid_type@:Expected number, received string",
);
check(
  parts,
  passed,
  formatResult(z.number().min(0).safeParse(-1)),
  "err:too_small@:Number must be greater than or equal to 0",
);
check(
  parts,
  passed,
  formatResult(z.number().max(10).safeParse(11)),
  "err:too_big@:Number must be less than or equal to 10",
);

check(parts, passed, formatResult(z.boolean().safeParse(true)), "ok:true");
check(
  parts,
  passed,
  formatResult(z.boolean().safeParse(1)),
  "err:invalid_type@:Expected boolean, received number",
);

check(parts, passed, formatResult(z.literal("admin").safeParse("admin")), 'ok:"admin"');
check(
  parts,
  passed,
  formatResult(z.literal("admin").safeParse("user")),
  'err:invalid_literal@:Invalid literal value, expected "admin"',
);
check(parts, passed, formatResult(z.literal(1).safeParse(1)), "ok:1");
check(parts, passed, formatResult(z.literal(true).safeParse(true)), "ok:true");

check(
  parts,
  passed,
  formatResult(z.array(z.string()).safeParse(["a", "b"])),
  'ok:["a","b"]',
);
check(
  parts,
  passed,
  formatResult(z.array(z.number().min(0)).safeParse([1, -1])),
  "err:too_small@1:Number must be greater than or equal to 0",
);
check(
  parts,
  passed,
  formatResult(z.array(z.string()).safeParse("x")),
  "err:invalid_type@:Expected array, received string",
);

check(
  parts,
  passed,
  formatResult(z.string().optional().safeParse(undefined)),
  "ok:undefined",
);
check(
  parts,
  passed,
  formatResult(z.string().optional().safeParse(null)),
  "err:invalid_type@:Expected string, received null",
);
check(
  parts,
  passed,
  formatResult(z.string().nullable().safeParse(null)),
  "ok:null",
);
check(
  parts,
  passed,
  formatResult(z.string().nullable().safeParse(undefined)),
  "err:invalid_type@:Required",
);

const User = z.object({
  email: z.string().email().min(5),
  age: z.number().min(0).max(150),
  admin: z.boolean(),
  role: z.literal("owner"),
  tags: z.array(z.string().min(1)).optional(),
  bio: z.string().nullable(),
});

check(
  parts,
  passed,
  formatResult(
    User.safeParse({
      email: "a@b.co",
      age: 20,
      admin: false,
      role: "owner",
      bio: null,
    }),
  ),
  'ok:{"email":"a@b.co","age":20,"admin":false,"role":"owner","bio":null}',
);
check(
  parts,
  passed,
  formatResult(
    User.safeParse({
      email: "test@example.com",
      age: 0,
      admin: true,
      role: "owner",
      tags: ["x"],
      bio: "hi",
    }),
  ),
  'ok:{"email":"test@example.com","age":0,"admin":true,"role":"owner","tags":["x"],"bio":"hi"}',
);
check(
  parts,
  passed,
  formatResult(
    User.safeParse({
      email: "x",
      age: 200,
      admin: "no",
      role: "guest",
      tags: [""],
      bio: 1,
    }),
  ),
  "err:invalid_string@email:Invalid email;too_small@email:String must contain at least 5 character(s);too_big@age:Number must be less than or equal to 150;invalid_type@admin:Expected boolean, received string;invalid_literal@role:Invalid literal value, expected \"owner\";too_small@tags/0:String must contain at least 1 character(s);invalid_type@bio:Expected string, received number",
);
check(
  parts,
  passed,
  formatResult(User.safeParse({})),
  'err:invalid_type@email:Required;invalid_type@age:Required;invalid_type@admin:Required;invalid_literal@role:Invalid literal value, expected "owner";invalid_type@bio:Required',
);
check(
  parts,
  passed,
  formatResult(z.object({ a: z.string() }).safeParse({ a: "x", b: 1 })),
  'ok:{"a":"x"}',
);

const parsed = User.parse({
  email: "ok@ex.com",
  age: 9,
  admin: false,
  role: "owner",
  bio: null,
});
check(parts, passed, `parse:${parsed.email}:${parsed.age}`, "parse:ok@ex.com:9");

let threw = 0;
try {
  User.parse({ email: "bad" });
} catch {
  threw = 1;
}
check(parts, passed, `throw:${threw}`, "throw:1");

const Color = z.enum(["red", "green"]);
check(parts, passed, formatResult(Color.safeParse("red")), 'ok:"red"');
check(
  parts,
  passed,
  formatResult(Color.safeParse("blue")),
  "err:invalid_enum_value@:Invalid enum value. Expected 'red' | 'green', received 'blue'",
);
check(
  parts,
  passed,
  formatResult(Color.safeParse(1)),
  "err:invalid_type@:Expected 'red' | 'green', received number",
);

const StrOrNum = z.union([z.string(), z.number()]);
check(parts, passed, formatResult(StrOrNum.safeParse("a")), 'ok:"a"');
check(parts, passed, formatResult(StrOrNum.safeParse(1)), "ok:1");
check(
  parts,
  passed,
  formatResult(StrOrNum.safeParse(true)),
  "err:invalid_union@:Invalid input",
);

const Shape = z.discriminatedUnion("type", [
  z.object({ type: z.literal("a"), v: z.string() }),
  z.object({ type: z.literal("b"), v: z.number() }),
]);
check(
  parts,
  passed,
  formatResult(Shape.safeParse({ type: "a", v: "x" })),
  'ok:{"type":"a","v":"x"}',
);
check(
  parts,
  passed,
  formatResult(Shape.safeParse({ type: "c", v: "x" })),
  "err:invalid_union_discriminator@type:Invalid discriminator value. Expected 'a' | 'b'",
);
check(
  parts,
  passed,
  formatResult(Shape.safeParse({ type: "a", v: 1 })),
  "err:invalid_type@v:Expected string, received number",
);

const Pair = z.tuple([z.string(), z.number()]);
check(parts, passed, formatResult(Pair.safeParse(["a", 1])), 'ok:["a",1]');
check(
  parts,
  passed,
  formatResult(Pair.safeParse(["a"])),
  "err:too_small@:Array must contain at least 2 element(s)",
);
check(
  parts,
  passed,
  formatResult(Pair.safeParse(["a", 1, true])),
  "err:too_big@:Array must contain at most 2 element(s)",
);
check(
  parts,
  passed,
  formatResult(Pair.safeParse(["a", "b"])),
  "err:invalid_type@1:Expected number, received string",
);

check(
  parts,
  passed,
  formatResult(z.string().default("hi").safeParse(undefined)),
  'ok:"hi"',
);
check(
  parts,
  passed,
  formatResult(z.string().default("hi").safeParse("x")),
  'ok:"x"',
);
check(
  parts,
  passed,
  formatResult(z.string().transform((s) => s.length).safeParse("abc")),
  "ok:3",
);
check(
  parts,
  passed,
  formatResult(z.string().transform((s) => s.length).safeParse(1)),
  "err:invalid_type@:Expected string, received number",
);

const Scores = z.record(z.number());
check(
  parts,
  passed,
  formatResult(Scores.safeParse({ a: 1, b: 2 })),
  'ok:{"a":1,"b":2}',
);
check(
  parts,
  passed,
  formatResult(Scores.safeParse({ a: "x" })),
  "err:invalid_type@a:Expected number, received string",
);
check(
  parts,
  passed,
  formatResult(Scores.safeParse("x")),
  "err:invalid_type@:Expected object, received string",
);

console.log(`zod:${passed.value}:${parts.join("|")}`);
