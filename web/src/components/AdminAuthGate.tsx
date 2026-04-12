import { useEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Spin } from 'antd';

import { fetchAdminAuthStatus, getAdminToken } from '../services/adminAuth';

export default function AdminAuthGate({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    let cancelled = false;
    void fetchAdminAuthStatus()
      .then((status) => {
        if (cancelled) {
          return;
        }
        if (status.auth_required) {
          const token = getAdminToken();
          if (!token) {
            const next = `${location.pathname}${location.search}`;
            navigate(`/login?next=${encodeURIComponent(next || '/traffic')}`, {
              replace: true,
            });
            return;
          }
        }
        setReady(true);
      })
      .catch(() => {
        // If status cannot be fetched, keep existing behavior (avoid misjudging offline/starting core as needing login).
        if (!cancelled) {
          setReady(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [location.pathname, location.search, navigate]);

  if (!ready) {
    return (
      <div style={{ display: 'grid', placeItems: 'center', height: '100vh' }}>
        <Spin />
      </div>
    );
  }
  return <>{children}</>;
}

