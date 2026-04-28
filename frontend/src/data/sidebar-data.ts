import { Project, User } from '../types/navigation';

export const projects: Project[] = [
  {
    name: 'Extrittio',
    environment: 'Development',
    icon: 'code',
  },
  {
    name: 'Extrittio',
    environment: 'Testing',
    icon: 'lab-test',
  },
  {
    name: 'Extrittio',
    environment: 'Production',
    icon: 'build',
  },
];

export const currentUser: User = {
  name: 'Patryk Kępa',
  email: 'opensource@extrittio.dev',
  avatar: '/avatars/shadcn.jpg',
};
