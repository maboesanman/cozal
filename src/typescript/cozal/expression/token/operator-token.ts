// 0x1...
export enum BooleanOperatorToken {
  // 0x1001... boolean operators
}
export enum IntegerOperatorToken {
  // 0x1010... integer operators
  AddInteger = 0x1010_0001_0000_0000,
  SubtractInteger = 0x1010_0001_0000_0001,
  MultiplyInteger = 0x1010_0001_0000_0010,
  DivideInteger = 0x1010_0001_0000_0011
}
export enum FloatOperatorToken {
  // 0x1011... float operators
  AddFloat = 0x1011_0001_0000_0000,
  SubtractFloat = 0x1011_0001_0000_0001,
  MultiplyFloat = 0x1011_0001_0000_0010,
  DivideFloat = 0x1011_0001_0000_0011
}
export enum StringOperatorToken {
  // 0x1100... string operators
}

export const OperatorToken = {
  ...BooleanOperatorToken,
  ...IntegerOperatorToken,
  ...FloatOperatorToken,
  ...StringOperatorToken
};

export type OperatorToken = BooleanOperatorToken
  | IntegerOperatorToken
  | FloatOperatorToken
  | StringOperatorToken;

export default OperatorToken;
