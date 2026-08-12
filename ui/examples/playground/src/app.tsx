import {
  NavLink,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from "react-router-dom";
import { ComponentGallery } from "./routes/component-gallery";
import { SiteBlogPage } from "./routes/site-blog-page";
import { SiteProductPage } from "./routes/site-product-page";
import { StressPage } from "./routes/stress";
import { WorkspaceDemo } from "./routes/workspace-demo";

const playgroundRoutes = [
  { label: "Product", href: "/site/product" },
  { label: "Workspace", href: "/workspace" },
  { label: "Blog", href: "/site/blog" },
  { label: "Components", href: "/components" },
  { label: "Stress", href: "/stress" },
];

const PlaygroundSwitcher = () => (
  <nav className="playground-switcher" aria-label="Playground demos">
    {playgroundRoutes.map((route) => (
      <NavLink key={route.href} to={route.href}>
        {route.label}
      </NavLink>
    ))}
  </nav>
);

export const App = () => {
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <>
      <Routes>
        <Route path="/" element={<Navigate to="/site/product" replace />} />
        <Route
          path="/site/product"
          element={
            <SiteProductPage
              currentPath={location.pathname}
              onNavigate={navigate}
            />
          }
        />
        <Route
          path="/site/blog"
          element={
            <SiteBlogPage
              currentPath={location.pathname}
              onNavigate={navigate}
            />
          }
        />
        <Route
          path="/components"
          element={
            <ComponentGallery
              currentPath={location.pathname}
              onNavigate={navigate}
            />
          }
        />
        <Route
          path="/stress"
          element={
            <StressPage currentPath={location.pathname} onNavigate={navigate} />
          }
        />
        <Route path="/workspace/*" element={<WorkspaceDemo />} />
      </Routes>
      <PlaygroundSwitcher />
    </>
  );
};
