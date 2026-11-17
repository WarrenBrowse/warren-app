import { Page } from 'playwright';

import { TestUtils } from '../../utils';
import { NavigationObjectModel } from '../navigation';

export class AntiCensorshipRouteObjectModel extends NavigationObjectModel {
  constructor(page: Page, utils: TestUtils) {
    super(page, utils);
  }
}
