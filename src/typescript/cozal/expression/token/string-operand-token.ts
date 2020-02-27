import Token, { getTokenType, getTokenReturnType } from "./token";

// 0b0100... string operands
export enum StateStringOperandToken {
  StateString = 0b0100_0000_0000_0000
}

export enum ConstantStringOperandToken {
  ConstantString = 0b0100_0000_0000_0001
}

export enum SystemStringOperandToken {
}

export const StringOperandToken = {
  ...StateStringOperandToken,
  ...ConstantStringOperandToken,
  ...SystemStringOperandToken
};

export type StringOperandToken = StateStringOperandToken
  | ConstantStringOperandToken
  | SystemStringOperandToken;

export function isStringOperandToken(token:
  Token): token is StringOperandToken
{
  return getTokenType(token) === "operand" &&
    getTokenReturnType(token) === "string";
}

export default StringOperandToken;
