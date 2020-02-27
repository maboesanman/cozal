import OperandToken from "./operand-token.ts";
import OperatorToken from "./operator-token.ts";

export const Token = {
  ...OperandToken,
  ...OperatorToken
};

export type Token = OperandToken | OperatorToken;
export default Token;
