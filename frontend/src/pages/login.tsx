import { useState } from 'react';
import {
  Card,
  Elevation,
  FormGroup,
  InputGroup,
  Button,
  Callout,
  H3,
  Icon,
} from '@blueprintjs/core';
import { useNavigate } from 'react-router-dom';
import { login } from '../api/auth';
import { useAuthStore } from '../stores/auth-store';
import './login.css';

export const Login = () => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const setToken = useAuthStore((s) => s.setToken);
  const navigate = useNavigate();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setIsLoading(true);

    try {
      const response = await login({ username, password });
      setToken(response.token);
      navigate('/', { replace: true });
    } catch {
      setError('Invalid username or password');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="login-page">
      <Card elevation={Elevation.THREE} className="login-card">
        <div className="login-header">
          <div className="login-logo">
            <Icon icon="cube" size={24} />
          </div>
          <H3 style={{ margin: 0 }}>Extrittio</H3>
          <p className="login-subtitle">Sign in to your IoT Hub</p>
        </div>

        <form onSubmit={handleSubmit}>
          {error && (
            <Callout intent="danger" icon="error" style={{ marginBottom: 16 }}>
              {error}
            </Callout>
          )}

          <FormGroup label="Username">
            <InputGroup
              leftIcon="user"
              placeholder="admin"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
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
            />
          </FormGroup>

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
      </Card>
    </div>
  );
};
