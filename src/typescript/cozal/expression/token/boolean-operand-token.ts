import Token, { getTokenType, getTokenReturnType } from "./token.ts";

// 0b0001... boolean operands
export enum StateBooleanOperandToken {
  StateBoolean = 0b0001_0000_0000_0000
}

export enum ConstantBooleanOperandToken {
  ConstantBoolean = 0b0001_0000_0000_0001
}

export enum SystemBooleanOperandToken {
}

export const BooleanOperandToken = {
  ...StateBooleanOperandToken,
  ...ConstantBooleanOperandToken,
  ...SystemBooleanOperandToken
};

export type BooleanOperandToken = StateBooleanOperandToken
  | ConstantBooleanOperandToken
  | SystemBooleanOperandToken;

export function isBooleanOperandToken(token:
  Token): token is BooleanOperandToken
{
  return getTokenType(token) === "operand" &&
    getTokenReturnType(token) === "boolean";
}

export default BooleanOperandToken;
