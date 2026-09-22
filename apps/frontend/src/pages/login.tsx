import { useState } from 'react';
import { FormGroup, InputGroup, Button, Callout, H3, Icon } from '@blueprintjs/core';
import { useNavigate } from 'react-router-dom';
import axios from 'axios';
import { login } from '../api/auth';
import { getDefaultRoutePath } from '../app/routes';
import { useAuthStore } from '../stores/auth-store';
import './login.css';

export const Login = () => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [tenantId, setTenantId] = useState('');
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const setSession = useAuthStore((s) => s.setSession);
  const navigate = useNavigate();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setIsLoading(true);

    try {
      const response = await login({
        username,
        password,
        ...(!__EXTRITTIO_EDGE__ && tenantId.trim() ? { tenant_id: tenantId.trim() } : {}),
      });
      setSession(response.user);
      navigate(getDefaultRoutePath(response.user.permissions), { replace: true });
    } catch (failure) {
      if (axios.isAxiosError(failure) && failure.response?.status === 401) {
        setError('Invalid username or password');
      } else if (axios.isAxiosError(failure) && !failure.response) {
        setError('Could not connect to the server. Check your connection and try again.');
      } else {
        setError('Sign in is unavailable right now. Please try again.');
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="login-page">
      <main className="login-panel">
        <section className="login-card" aria-labelledby="login-heading">
          <div className="login-header">
            <div className="login-logo" aria-hidden="true">
              <Icon icon="cube" size={20} />
            </div>
            <span className="login-product-name">Extrittio</span>
          </div>

          <div className="login-intro">
            <H3 id="login-heading">Sign in</H3>
            <p className="login-subtitle">Access your IoT Hub</p>
          </div>

          <form onSubmit={handleSubmit} className="login-form">
            {error && (
              <Callout intent="danger" icon="error" className="login-error">
                {error}
              </Callout>
            )}

            <FormGroup label="Username">
              <InputGroup
                leftIcon="user"
                placeholder="admin"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoComplete="username"
                autoFocus
              />
            </FormGroup>

            <FormGroup label="Password">
              <InputGroup
                leftIcon="lock"
                type="password"
                placeholder="Password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
              />
            </FormGroup>

            {!__EXTRITTIO_EDGE__ ? (
              <FormGroup label="Tenant (optional)">
                <InputGroup
                  leftIcon="office"
                  placeholder="default"
                  value={tenantId}
                  onChange={(e) => setTenantId(e.target.value)}
                />
              </FormGroup>
            ) : null}

            <Button
              type="submit"
              intent="primary"
              fill
              loading={isLoading}
              disabled={!username.trim() || !password.trim()}
            >
              Sign In
            </Button>
          </form>
        </section>
      </main>
    </div>
  );
};
