import React from 'react';

interface AppProps {
  showMessage?: boolean;
  user?: {
    name: string;
    verified: boolean;
    avatar: string;
  };
  settings?: {
    showProfile: boolean;
  };
}

function App({ showMessage, user, settings }: AppProps) {
  return (
    <div>
      <h1>SWC Condition Plugin Demo</h1>
      
      {/* Basic condition in JSX */}
      <Condition if={showMessage}>
        <p>Hello World! This message is conditionally rendered.</p>
      </Condition>

      {/* Complex nested conditions */}
      <Condition if={user}>
        <header>
          <h2>Welcome {user.name}</h2>
          <Condition if={settings?.showProfile}>
            <div className="profile">
              <img src={user.avatar} alt="Avatar" />
              <Condition if={user.verified}>
                <span className="verified">✓ Verified</span>
              </Condition>
            </div>
          </Condition>
        </header>
      </Condition>

      {/* Assignment context */}
      {(() => {
        const element = <Condition if={showMessage}>
          <span>This is in an assignment context</span>
        </Condition>;
        return element;
      })()}
    </div>
  );
}

function SimpleComponent({ show }: { show: boolean }) {
  return <Condition if={show}>
    <div>This is in a return context</div>
  </Condition>;
}

export default App;
