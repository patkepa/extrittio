import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { NonIdealState, Button } from '@blueprintjs/core';

export interface ErrorBoundaryProps {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  resetKey: number;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, State> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, resetKey: 0 };
  }

  static getDerivedStateFromError(_error: Error): Partial<State> {
    return { hasError: true };
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
