// 0x0001... boolean operands
export enum StateBooleanOperandToken {
  StateBoolean = 0x0001_0000_0000_0000
}

export enum ConstantBooleanOperandToken {
  ConstantBoolean = 0x0001_0000_0000_0001
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

export default BooleanOperandToken;
