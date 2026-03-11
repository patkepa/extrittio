import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { NonIdealState, Button } from '@blueprintjs/core';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  resetKey: number;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, resetKey: 0 };
  }

  static getDerivedStateFromError(): State {
    return { hasError: true, resetKey: 0 };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <NonIdealState
          icon="error"
          title="Something went wrong"
          description="An unexpected error occurred in this section."
          action={
            <Button
              intent="primary"
              icon="refresh"
              onClick={() => this.setState((s) => ({ hasError: false, resetKey: s.resetKey + 1 }))}
            >
              Try Again
            </Button>
          }
        />
      );
    }
    return <div key={this.state.resetKey}>{this.props.children}</div>;
  }
}
