// 0x0001... Integer operands
export enum StateIntegerOperandToken {
  StateInteger = 0x0010_0000_0000_0000
}

export enum ConstantIntegerOperandToken {
  ConstantInteger = 0x0010_0000_0000_0001
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

export default IntegerOperandToken;
