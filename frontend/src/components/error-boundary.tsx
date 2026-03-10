import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { NonIdealState, Button } from '@blueprintjs/core';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): State {
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
              onClick={() => this.setState({ hasError: false })}
            >
              Try Again
            </Button>
          }
        />
      );
    }
    return this.props.children;
  }
}
