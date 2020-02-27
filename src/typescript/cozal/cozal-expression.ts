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
} from "./expression/expression";
import {
  add,
  subtract,
  multiply,
  divide
} from "./expression/math/binary-operations";

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
    divide
  }
};

export default ExpressionAPI;
