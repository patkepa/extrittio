import { H3, NonIdealState } from '@blueprintjs/core';

export const HelpCenter = () => {
  return (
    <div className="help-center-page">
      <div className="page-header">
        <H3>Help Center</H3>
      </div>
      <NonIdealState
        icon="help"
        title="Help Center"
        description="Documentation and support resources coming soon."
      />
    </div>
  );
};
