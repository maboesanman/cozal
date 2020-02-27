import Exp, {
  FloatExpression as FloatExp,
  fromOperator
} from "../expression.ts";
import Token from "../token/token.ts";

export function sin<S>(arg: FloatExp<S>): FloatExp<S> {
  return fromOperator(Token.Sin, arg) as FloatExp<S>;
}

export function cos<S>(arg: FloatExp<S>): FloatExp<S> {
  return fromOperator(Token.Cos, arg) as FloatExp<S>;
}

export function tan<S>(arg: FloatExp<S>): FloatExp<S> {
  return fromOperator(Token.Tan, arg) as FloatExp<S>;
}
