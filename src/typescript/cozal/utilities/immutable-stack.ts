interface StackNode<T> {
  value: T;
  next?: StackNode<T>;
}

export default class Stack<T> {
  readonly head?: StackNode<T>;

  private static fromHead<T>(head?: StackNode<T>): Stack<T> {
    // manually assign to readonly property. this is an alternate private constructor.
    const result = new Stack<T>() as any;
    result.head = head;
    return result as Stack<T>;
  }

  public push(value: T): Stack<T> {
    return Stack.fromHead({ value, next: this.head });
  }

  public pop(): Stack<T> {
    return Stack.fromHead(this.head?.next);
  }

  public peek() {
    return this.head?.value;
  }
}
