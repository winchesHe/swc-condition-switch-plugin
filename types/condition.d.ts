declare namespace JSX {
  interface IntrinsicElements {
    Condition: {
      if: any;
      children?: React.ReactNode;
    };
    Switch: {
      shortCircuit?: boolean;
      children?: React.ReactNode;
    };
  }
}

declare namespace Switch {
  interface Case {
    if: any;
    children?: React.ReactNode;
  }
}

export {};
