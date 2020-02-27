// 0x0011... float operands
export enum StateFloatOperandToken {
  StateFloat = 0x0011_0000_0000_0000
}

export enum ConstantFloatOperandToken {
  ConstantFloat = 0x0011_0000_0000_0001
}

export enum SystemFloatOperandToken {
  Time = 0x0011_0100_0000_0000
}

export const FloatOperandToken = {
  ...StateFloatOperandToken,
  ...ConstantFloatOperandToken,
  ...SystemFloatOperandToken
};

export type FloatOperandToken = StateFloatOperandToken
  | ConstantFloatOperandToken
  | SystemFloatOperandToken;

export default FloatOperandToken;
