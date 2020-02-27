import Token, { getTokenType, getTokenReturnType } from "./token.ts";

// 0b0011... float operands
export enum StateFloatOperandToken {
  StateFloat = 0b0011_0000_0000_0000
}

export enum ConstantFloatOperandToken {
  ConstantFloat = 0b0011_0000_0000_0001
}

export enum SystemFloatOperandToken {
  Time = 0b0011_0100_0000_0000
}

export const FloatOperandToken = {
  ...StateFloatOperandToken,
  ...ConstantFloatOperandToken,
  ...SystemFloatOperandToken
};

export type FloatOperandToken = StateFloatOperandToken
  | ConstantFloatOperandToken
  | SystemFloatOperandToken;

export function isFloatOperandToken(token: Token): token is FloatOperandToken {
  return getTokenType(token) === "operand" &&
    getTokenReturnType(token) === "float";
}

export default FloatOperandToken;
