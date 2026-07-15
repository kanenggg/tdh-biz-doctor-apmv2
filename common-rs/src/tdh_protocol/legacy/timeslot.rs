use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum DoctorAvailableResult {
    #[serde(rename = "GET_AVAILABLE_DOCTOR_SCHEDULES_SUCCEEDED")]
    Success { schedules: Vec<AvailableSchedule> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableSchedule {
    pub slot_id: i64,
    pub slot_start_time: i64,
    pub slot_end_time: i64,
}

// {
//   "__type": "GET_AVAILABLE_DOCTOR_SCHEDULES_SUCCEEDED",
//   "schedules": [
//     {
//       "slotId": 794223,
//       "slotStartTime": 1774333200,
//       "slotEndTime": 1774334100
//     },
//     {
//       "slotId": 794224,
//       "slotStartTime": 1774334400,
//       "slotEndTime": 1774335300
//     },
//     {
//       "slotId": 794381,
//       "slotStartTime": 1774400400,
//       "slotEndTime": 1774401300
//     },
//     {
//       "slotId": 794382,
//       "slotStartTime": 1774401600,
//       "slotEndTime": 1774402500
//     },
//     {
//       "slotId": 794383,
//       "slotStartTime": 1774402800,
//       "slotEndTime": 1774403700
//     },
//     {
//       "slotId": 794384,
//       "slotStartTime": 1774404000,
//       "slotEndTime": 1774404900
//     },
//     {
//       "slotId": 794385,
//       "slotStartTime": 1774405200,
//       "slotEndTime": 1774406100
//     },
//     {
//       "slotId": 794386,
//       "slotStartTime": 1774406400,
//       "slotEndTime": 1774407300
//     },
//     {
//       "slotId": 794387,
//       "slotStartTime": 1774407600,
//       "slotEndTime": 1774408500
//     },
//     {
//       "slotId": 794388,
//       "slotStartTime": 1774408800,
//       "slotEndTime": 1774409700
//     },
//     {
//       "slotId": 794389,
//       "slotStartTime": 1774410000,
//       "slotEndTime": 1774410900
//     },
//     {
//       "slotId": 794390,
//       "slotStartTime": 1774411200,
//       "slotEndTime": 1774412100
//     },
//     {
//       "slotId": 794391,
//       "slotStartTime": 1774412400,
//       "slotEndTime": 1774413300
//     },
//     {
//       "slotId": 794392,
//       "slotStartTime": 1774413600,
//       "slotEndTime": 1774414500
//     },
//     {
//       "slotId": 794393,
//       "slotStartTime": 1774414800,
//       "slotEndTime": 1774415700
//     },
//     {
//       "slotId": 794394,
//       "slotStartTime": 1774416000,
//       "slotEndTime": 1774416900
//     },
//     {
//       "slotId": 794395,
//       "slotStartTime": 1774417200,
//       "slotEndTime": 1774418100
//     },
//     {
//       "slotId": 794396,
//       "slotStartTime": 1774418400,
//       "slotEndTime": 1774419300
//     },
//     {
//       "slotId": 794397,
//       "slotStartTime": 1774419600,
//       "slotEndTime": 1774420500
//     },
//     {
//       "slotId": 794398,
//       "slotStartTime": 1774420800,
//       "slotEndTime": 1774421700
//     },
//     {
//       "slotId": 794399,
//       "slotStartTime": 1774422000,
//       "slotEndTime": 1774422900
//     },
//     {
//       "slotId": 794400,
//       "slotStartTime": 1774423200,
//       "slotEndTime": 1774424100
//     },
//     {
//       "slotId": 794401,
//       "slotStartTime": 1774424400,
//       "slotEndTime": 1774425300
//     },
//     {
//       "slotId": 794402,
//       "slotStartTime": 1774425600,
//       "slotEndTime": 1774426500
//     },
//     {
//       "slotId": 794403,
//       "slotStartTime": 1774426800,
//       "slotEndTime": 1774427700
//     },
//     {
//       "slotId": 794404,
//       "slotStartTime": 1774428000,
//       "slotEndTime": 1774428900
//     },
//     {
//       "slotId": 794405,
//       "slotStartTime": 1774429200,
//       "slotEndTime": 1774430100
//     },
//     {
//       "slotId": 794406,
//       "slotStartTime": 1774430400,
//       "slotEndTime": 1774431300
//     },
//     {
//       "slotId": 794407,
//       "slotStartTime": 1774431600,
//       "slotEndTime": 1774432500
//     },
//     {
//       "slotId": 794408,
//       "slotStartTime": 1774432800,
//       "slotEndTime": 1774433700
//     },
//     {
//       "slotId": 794409,
//       "slotStartTime": 1774434000,
//       "slotEndTime": 1774434900
//     },
//     {
//       "slotId": 794410,
//       "slotStartTime": 1774435200,
//       "slotEndTime": 1774436100
//     },
//     {
//       "slotId": 794129,
//       "slotStartTime": 1774846800,
//       "slotEndTime": 1774847700
//     },
//     {
//       "slotId": 794130,
//       "slotStartTime": 1774848000,
//       "slotEndTime": 1774848900
//     },
//     {
//       "slotId": 794131,
//       "slotStartTime": 1774849200,
//       "slotEndTime": 1774850100
//     },
//     {
//       "slotId": 794225,
//       "slotStartTime": 1774918800,
//       "slotEndTime": 1774919700
//     },
//     {
//       "slotId": 794226,
//       "slotStartTime": 1774920000,
//       "slotEndTime": 1774920900
//     },
//     {
//       "slotId": 794227,
//       "slotStartTime": 1774921200,
//       "slotEndTime": 1774922100
//     },
//     {
//       "slotId": 794228,
//       "slotStartTime": 1774922400,
//       "slotEndTime": 1774923300
//     },
//     {
//       "slotId": 794229,
//       "slotStartTime": 1774923600,
//       "slotEndTime": 1774924500
//     },
//     {
//       "slotId": 794230,
//       "slotStartTime": 1774924800,
//       "slotEndTime": 1774925700
//     },
//     {
//       "slotId": 794231,
//       "slotStartTime": 1774926000,
//       "slotEndTime": 1774926900
//     },
//     {
//       "slotId": 794232,
//       "slotStartTime": 1774927200,
//       "slotEndTime": 1774928100
//     },
//     {
//       "slotId": 794233,
//       "slotStartTime": 1774928400,
//       "slotEndTime": 1774929300
//     },
//     {
//       "slotId": 794234,
//       "slotStartTime": 1774929600,
//       "slotEndTime": 1774930500
//     },
//     {
//       "slotId": 794235,
//       "slotStartTime": 1774930800,
//       "slotEndTime": 1774931700
//     },
//     {
//       "slotId": 794236,
//       "slotStartTime": 1774932000,
//       "slotEndTime": 1774932900
//     },
//     {
//       "slotId": 794237,
//       "slotStartTime": 1774933200,
//       "slotEndTime": 1774934100
//     },
//     {
//       "slotId": 794238,
//       "slotStartTime": 1774934400,
//       "slotEndTime": 1774935300
//     },
//     {
//       "slotId": 794239,
//       "slotStartTime": 1774935600,
//       "slotEndTime": 1774936500
//     },
//     {
//       "slotId": 794240,
//       "slotStartTime": 1774936800,
//       "slotEndTime": 1774937700
//     },
//     {
//       "slotId": 794241,
//       "slotStartTime": 1774938000,
//       "slotEndTime": 1774938900
//     },
//     {
//       "slotId": 794242,
//       "slotStartTime": 1774939200,
//       "slotEndTime": 1774940100
//     },
//     {
//       "slotId": 794411,
//       "slotStartTime": 1775005200,
//       "slotEndTime": 1775006100
//     },
//     {
//       "slotId": 794412,
//       "slotStartTime": 1775006400,
//       "slotEndTime": 1775007300
//     },
//     {
//       "slotId": 794413,
//       "slotStartTime": 1775007600,
//       "slotEndTime": 1775008500
//     },
//     {
//       "slotId": 794414,
//       "slotStartTime": 1775008800,
//       "slotEndTime": 1775009700
//     },
//     {
//       "slotId": 794415,
//       "slotStartTime": 1775010000,
//       "slotEndTime": 1775010900
//     },
//     {
//       "slotId": 794416,
//       "slotStartTime": 1775011200,
//       "slotEndTime": 1775012100
//     },
//     {
//       "slotId": 794417,
//       "slotStartTime": 1775012400,
//       "slotEndTime": 1775013300
//     },
//     {
//       "slotId": 794418,
//       "slotStartTime": 1775013600,
//       "slotEndTime": 1775014500
//     },
//     {
//       "slotId": 794419,
//       "slotStartTime": 1775014800,
//       "slotEndTime": 1775015700
//     },
//     {
//       "slotId": 794420,
//       "slotStartTime": 1775016000,
//       "slotEndTime": 1775016900
//     },
//     {
//       "slotId": 794421,
//       "slotStartTime": 1775017200,
//       "slotEndTime": 1775018100
//     },
//     {
//       "slotId": 794422,
//       "slotStartTime": 1775018400,
//       "slotEndTime": 1775019300
//     },
//     {
//       "slotId": 794423,
//       "slotStartTime": 1775019600,
//       "slotEndTime": 1775020500
//     },
//     {
//       "slotId": 794424,
//       "slotStartTime": 1775020800,
//       "slotEndTime": 1775021700
//     },
//     {
//       "slotId": 794425,
//       "slotStartTime": 1775022000,
//       "slotEndTime": 1775022900
//     },
//     {
//       "slotId": 794426,
//       "slotStartTime": 1775023200,
//       "slotEndTime": 1775024100
//     },
//     {
//       "slotId": 794427,
//       "slotStartTime": 1775024400,
//       "slotEndTime": 1775025300
//     },
//     {
//       "slotId": 794428,
//       "slotStartTime": 1775025600,
//       "slotEndTime": 1775026500
//     },
//     {
//       "slotId": 794429,
//       "slotStartTime": 1775026800,
//       "slotEndTime": 1775027700
//     },
//     {
//       "slotId": 794430,
//       "slotStartTime": 1775028000,
//       "slotEndTime": 1775028900
//     },
//     {
//       "slotId": 794431,
//       "slotStartTime": 1775029200,
//       "slotEndTime": 1775030100
//     },
//     {
//       "slotId": 794432,
//       "slotStartTime": 1775030400,
//       "slotEndTime": 1775031300
//     },
//     {
//       "slotId": 794433,
//       "slotStartTime": 1775031600,
//       "slotEndTime": 1775032500
//     },
//     {
//       "slotId": 794434,
//       "slotStartTime": 1775032800,
//       "slotEndTime": 1775033700
//     },
//     {
//       "slotId": 794435,
//       "slotStartTime": 1775034000,
//       "slotEndTime": 1775034900
//     },
//     {
//       "slotId": 794436,
//       "slotStartTime": 1775035200,
//       "slotEndTime": 1775036100
//     },
//     {
//       "slotId": 794437,
//       "slotStartTime": 1775036400,
//       "slotEndTime": 1775037300
//     },
//     {
//       "slotId": 794438,
//       "slotStartTime": 1775037600,
//       "slotEndTime": 1775038500
//     },
//     {
//       "slotId": 794439,
//       "slotStartTime": 1775038800,
//       "slotEndTime": 1775039700
//     },
//     {
//       "slotId": 794440,
//       "slotStartTime": 1775040000,
//       "slotEndTime": 1775040900
//     },
//     {
//       "slotId": 794132,
//       "slotStartTime": 1775451600,
//       "slotEndTime": 1775452500
//     },
//     {
//       "slotId": 794133,
//       "slotStartTime": 1775452800,
//       "slotEndTime": 1775453700
//     },
//     {
//       "slotId": 794134,
//       "slotStartTime": 1775454000,
//       "slotEndTime": 1775454900
//     },
//     {
//       "slotId": 794243,
//       "slotStartTime": 1775523600,
//       "slotEndTime": 1775524500
//     },
//     {
//       "slotId": 794244,
//       "slotStartTime": 1775524800,
//       "slotEndTime": 1775525700
//     },
//     {
//       "slotId": 794245,
//       "slotStartTime": 1775526000,
//       "slotEndTime": 1775526900
//     },
//     {
//       "slotId": 794246,
//       "slotStartTime": 1775527200,
//       "slotEndTime": 1775528100
//     },
//     {
//       "slotId": 794247,
//       "slotStartTime": 1775528400,
//       "slotEndTime": 1775529300
//     },
//     {
//       "slotId": 794248,
//       "slotStartTime": 1775529600,
//       "slotEndTime": 1775530500
//     },
//     {
//       "slotId": 794249,
//       "slotStartTime": 1775530800,
//       "slotEndTime": 1775531700
//     },
//     {
//       "slotId": 794250,
//       "slotStartTime": 1775532000,
//       "slotEndTime": 1775532900
//     },
//     {
//       "slotId": 794251,
//       "slotStartTime": 1775533200,
//       "slotEndTime": 1775534100
//     },
//     {
//       "slotId": 794252,
//       "slotStartTime": 1775534400,
//       "slotEndTime": 1775535300
//     },
//     {
//       "slotId": 794253,
//       "slotStartTime": 1775535600,
//       "slotEndTime": 1775536500
//     },
//     {
//       "slotId": 794254,
//       "slotStartTime": 1775536800,
//       "slotEndTime": 1775537700
//     },
//     {
//       "slotId": 794255,
//       "slotStartTime": 1775538000,
//       "slotEndTime": 1775538900
//     },
//     {
//       "slotId": 794256,
//       "slotStartTime": 1775539200,
//       "slotEndTime": 1775540100
//     },
//     {
//       "slotId": 794257,
//       "slotStartTime": 1775540400,
//       "slotEndTime": 1775541300
//     },
//     {
//       "slotId": 794258,
//       "slotStartTime": 1775541600,
//       "slotEndTime": 1775542500
//     },
//     {
//       "slotId": 794259,
//       "slotStartTime": 1775542800,
//       "slotEndTime": 1775543700
//     },
//     {
//       "slotId": 794260,
//       "slotStartTime": 1775544000,
//       "slotEndTime": 1775544900
//     }
//   ]
// }
