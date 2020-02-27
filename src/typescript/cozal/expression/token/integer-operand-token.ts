import Token, { getTokenType, getTokenReturnType } from "./token.ts";

// 0b0001... Integer operands
export enum StateIntegerOperandToken {
  StateInteger = 0b0010_0000_0000_0000
}

export enum ConstantIntegerOperandToken {
  ConstantInteger = 0b0010_0000_0000_0001
}

export enum SystemIntegerOperandToken {
}

export const IntegerOperandToken = {
  ...StateIntegerOperandToken,
  ...ConstantIntegerOperandToken,
  ...SystemIntegerOperandToken
};

export type IntegerOperandToken = StateIntegerOperandToken
  | ConstantIntegerOperandToken
  | SystemIntegerOperandToken;

export function isIntegerOperandToken(token:
  Token): token is IntegerOperandToken
{
  return getTokenType(token) === "operand" &&
    getTokenReturnType(token) === "float";
}

export default IntegerOperandToken;
