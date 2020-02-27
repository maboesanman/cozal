import BooleanOperandToken from "./boolean-operand-token";
import IntegerOperandToken from "./integer-operand-token";
import FloatOperandToken from "./float-operand-token";
import StringOperandToken from "./string-operand-token";
import Token, { getTokenType } from "./token";

// 0b0...
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

export function isOperandToken(token: Token): token is OperandToken {
  return getTokenType(token) === "operand";
}

export default OperandToken;
