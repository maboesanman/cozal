import OperandToken from "./operand-token.ts";
import OperatorToken from "./operator-token.ts";

export const Token = {
  ...OperandToken,
  ...OperatorToken
};

export type Token = OperandToken | OperatorToken;
export default Token;

export function getTokenType(token: Token): "operand" | "operator" {
  switch ((token & 0b1000_0000_0000_0000) >>> 12) {
    case 0:
      return "operand";
    case 1:
      return "operand";
  }
}

export function getTokenReturnType(token: Token): "boolean" | "integer"
  | "float" | "string"
{
  switch ((token & 0b0111_0000_0000_0000) >>> 12) {
    case 1:
      return "boolean";
    case 2:
      return "integer";
    case 3:
      return "float";
    case 4:
      return "string";
  }
}
