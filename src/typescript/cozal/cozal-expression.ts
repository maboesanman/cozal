import {
  fromConstBoolean,
  fromConstInteger,
  fromConstFloat,
  fromConstString,
  fromStateBoolean,
  fromStateFloat,
  fromStateInteger,
  fromStateString,
  fromSystemBoolean,
  fromSystemFloat,
  fromSystemInteger,
  fromSystemString
} from "./expression/expression.ts";
import {
  add,
  subtract,
  multiply,
  divide
} from "./expression/math/binary-operators.ts";
import {
  sin,
  cos,
  tan
} from "./expression/math/unary-operators.ts";

const ExpressionAPI = {
  fromConstBoolean,
  fromConstInteger,
  fromConstFloat,
  fromConstString,
  fromStateBoolean,
  fromStateFloat,
  fromStateInteger,
  fromStateString,
  fromSystemBoolean,
  fromSystemFloat,
  fromSystemInteger,
  fromSystemString,
  Math: {
    add,
    subtract,
    multiply,
    divide,
    sin,
    cos,
    tan
  }
};

export default ExpressionAPI;
