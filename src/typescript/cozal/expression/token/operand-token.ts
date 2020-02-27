import BooleanOperandToken from "./boolean-operand-token";
import IntegerOperandToken from "./integer-operand-token";
import FloatOperandToken from "./float-operand-token";
import StringOperandToken from "./string-operand-token";

// 0x0...
export const OperandToken = {
  ...BooleanOperandToken,
  ...IntegerOperandToken,
  ...FloatOperandToken,
  ...StringOperandToken
};

export type OperandToken = BooleanOperandToken
  | IntegerOperandToken
  | FloatOperandToken
  | StringOperandToken;

export default OperandToken;
