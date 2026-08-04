import { useSelector } from '../../store';

export const useVersionSupported = () => {
  return { supported: useSelector((state) => state.version.supported) };
};
