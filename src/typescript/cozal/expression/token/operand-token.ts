import BooleanOperandToken from "./boolean-operand-token.ts";
import IntegerOperandToken from "./integer-operand-token.ts";
import FloatOperandToken from "./float-operand-token.ts";
import StringOperandToken from "./string-operand-token.ts";
import Token, { getTokenType } from "./token.ts";

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
