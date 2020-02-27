// 0x0100... string operands
export enum StateStringOperandToken {
  StateString = 0x0100_0000_0000_0000
}

export enum ConstantStringOperandToken {
  ConstantString = 0x0100_0000_0000_0001
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

export default StringOperandToken;
